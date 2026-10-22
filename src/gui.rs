use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::process::Command as ProcessCommand;
use std::rc::Rc;

use gtk::prelude::*;
use gtk4 as gtk;

use crate::backend::diagnostics::{run_diagnostics, Check, CheckState};
use crate::backend::drivers::{recommend_print_package, recommend_scanner_package};
use crate::backend::packages::{
    aur_helper, enable_services, install_base_packages, install_packages,
};
use crate::backend::printers::{
    add_printer, discover_printers, list_queues, test_print, DiscoveredPrinter, InstalledPrinter,
    PrinterOrigin, PrinterTransport,
};
use crate::backend::scanners::{default_scan_path, discover_scanners, run_test_scan, Scanner};
use crate::backend::shell::SystemRunner;

const STEP_NAMES: [&str; 4] = ["diagnostics", "printers", "scanners", "finish"];

#[derive(Clone)]
struct UiState {
    printers: Rc<RefCell<Vec<DiscoveredPrinter>>>,
    scanners: Rc<RefCell<Vec<Scanner>>>,
    selected_printer: Rc<Cell<Option<usize>>>,
    selected_scanner: Rc<Cell<Option<usize>>>,
    scan_output: Rc<RefCell<Option<PathBuf>>>,
}

impl UiState {
    fn new() -> Self {
        Self {
            printers: Rc::new(RefCell::new(Vec::new())),
            scanners: Rc::new(RefCell::new(Vec::new())),
            selected_printer: Rc::new(Cell::new(None)),
            selected_scanner: Rc::new(Cell::new(None)),
            scan_output: Rc::new(RefCell::new(None)),
        }
    }
}

fn persian_number(value: usize) -> String {
    value
        .to_string()
        .chars()
        .map(|digit| match digit {
            '0'..='9' => char::from_u32('۰' as u32 + (digit as u32 - '0' as u32)).unwrap_or(digit),
            _ => digit,
        })
        .collect()
}

fn run_background<T, F, G>(work: F, done: G)
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
    G: FnOnce(T) + 'static,
{
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(work());
    });

    let mut done = Some(done);
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(50), move || match receiver
        .try_recv()
    {
        Ok(value) => {
            if let Some(done) = done.take() {
                done(value);
            }
            gtk::glib::ControlFlow::Break
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => gtk::glib::ControlFlow::Continue,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => gtk::glib::ControlFlow::Break,
    });
}

pub fn run() {
    let app = gtk::Application::builder()
        .application_id("io.github.thegreatazizi.linuprint")
        .build();
    app.connect_activate(build_ui);
    let _ = app.run_with_args(&["linuprint"]);
}

fn build_ui(app: &gtk::Application) {
    gtk::Widget::set_default_direction(gtk::TextDirection::Rtl);
    let state = UiState::new();

    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::SlideLeftRight)
        .vexpand(true)
        .hexpand(true)
        .build();
    stack.add_named(&build_diagnostics_page(), Some("diagnostics"));
    stack.add_named(&build_printer_page(&state), Some("printers"));
    stack.add_named(&build_scanner_page(&state), Some("scanners"));
    stack.add_named(&build_finish_page(), Some("finish"));

    let back = gtk::Button::with_label("مرحله قبل");
    let next = gtk::Button::with_label("مرحله بعد");
    let progress = gtk::Label::new(None);
    progress.set_hexpand(true);
    progress.set_halign(gtk::Align::Center);

    let navigation = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    navigation.set_margin_top(10);
    navigation.set_margin_bottom(10);
    navigation.set_margin_start(16);
    navigation.set_margin_end(16);
    navigation.append(&back);
    navigation.append(&progress);
    navigation.append(&next);

    let step = Rc::new(Cell::new(0usize));
    update_navigation(&stack, &back, &next, &progress, 0);
    {
        let stack = stack.clone();
        let back_button = back.clone();
        let next_button = next.clone();
        let progress_label = progress.clone();
        let step = step.clone();
        back.connect_clicked(move |_| {
            let current = step.get().saturating_sub(1);
            step.set(current);
            update_navigation(&stack, &back_button, &next_button, &progress_label, current);
        });
    }
    {
        let stack = stack.clone();
        let back_button = back.clone();
        let next_button = next.clone();
        let progress_label = progress.clone();
        let step = step.clone();
        next.connect_clicked(move |_| {
            let current = (step.get() + 1).min(STEP_NAMES.len() - 1);
            step.set(current);
            update_navigation(&stack, &back_button, &next_button, &progress_label, current);
        });
    }

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&stack);
    root.append(&navigation);

    let header = gtk::HeaderBar::new();
    let title = gtk::Label::new(Some("LinuPrint — ویزارد چاپگر و اسکنر"));
    title.add_css_class("title");
    header.set_title_widget(Some(&title));

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("LinuPrint — ویزارد چاپگر و اسکنر")
        .default_width(900)
        .default_height(700)
        .build();
    window.set_titlebar(Some(&header));
    window.set_child(Some(&root));
    window.present();
}

fn update_navigation(
    stack: &gtk::Stack,
    back: &gtk::Button,
    next: &gtk::Button,
    progress: &gtk::Label,
    step: usize,
) {
    stack.set_visible_child_name(STEP_NAMES[step]);
    back.set_sensitive(step > 0);
    next.set_sensitive(step + 1 < STEP_NAMES.len());
    progress.set_text(&format!(
        "مرحله {} از {}",
        persian_number(step + 1),
        persian_number(STEP_NAMES.len())
    ));
}

fn titled_page(title: &str, subtitle: &str) -> (gtk::Box, gtk::Box) {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 10);
    page.set_margin_top(14);
    page.set_margin_bottom(14);
    page.set_margin_start(24);
    page.set_margin_end(24);

    let title_label = gtk::Label::builder()
        .label(title)
        .halign(gtk::Align::Start)
        .wrap(true)
        .build();
    title_label.add_css_class("title-2");

    let subtitle_label = gtk::Label::builder()
        .label(subtitle)
        .halign(gtk::Align::Start)
        .wrap(true)
        .build();
    subtitle_label.add_css_class("dim-label");

    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.set_margin_top(6);

    page.append(&title_label);
    page.append(&subtitle_label);
    page.append(&content);
    (page, content)
}

fn scroll_page(content: &gtk::Box) -> gtk::Widget {
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .child(content)
        .build();
    let widget: gtk::Widget = scroller.upcast();
    widget
}

fn set_status(label: &gtk::Label, message: &str, kind: Option<&str>) {
    for class in ["error", "warning", "success"] {
        label.remove_css_class(class);
    }
    if let Some(class) = kind {
        label.add_css_class(class);
    }
    label.set_text(message);
}

fn build_diagnostics_page() -> gtk::Widget {
    let (page, content) = titled_page(
        "۱. بررسی پیش‌نیازها",
        "وضعیت CUPS، Avahi، SANE و دسترسی مدیریتی بررسی می‌شود. هر مورد را می‌توانید دوباره بررسی یا رفع کنید.",
    );

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let refresh_button = gtk::Button::with_label("بررسی مجدد");
    let fix_button = gtk::Button::with_label("فعال‌سازی سرویس‌ها");
    let install_button = gtk::Button::with_label("نصب بسته‌های پایه");
    actions.append(&refresh_button);
    actions.append(&fix_button);
    actions.append(&install_button);
    content.append(&actions);

    let status = gtk::Label::new(None);
    status.set_halign(gtk::Align::Start);
    status.set_wrap(true);
    status.add_css_class("dim-label");
    content.append(&status);

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    content.append(&list);

    load_diagnostics(&list, &status);

    {
        let list = list.clone();
        let status = status.clone();
        refresh_button.connect_clicked(move |_| {
            load_diagnostics(&list, &status);
        });
    }
    {
        let list = list.clone();
        let status = status.clone();
        let refresh_action = refresh_button.clone();
        let fix_action = fix_button.clone();
        let install_action = install_button.clone();
        fix_button.connect_clicked(move |_| {
            let buttons = [
                refresh_action.clone(),
                fix_action.clone(),
                install_action.clone(),
            ];
            for button in &buttons {
                button.set_sensitive(false);
            }
            set_status(&status, "در حال فعال‌سازی سرویس‌ها...", None);

            let list = list.clone();
            let status = status.clone();
            run_background(
                || {
                    let outcome = enable_services(&SystemRunner, true);
                    let checks = run_diagnostics(&SystemRunner);
                    (outcome, checks)
                },
                move |(outcome, checks)| {
                    fill_diagnostics(&list, &checks);
                    match outcome {
                        Ok(message) => set_status(&status, &message, Some("success")),
                        Err(error) => set_status(&status, &error, Some("error")),
                    }
                    for button in &buttons {
                        button.set_sensitive(true);
                    }
                },
            );
        });
    }
    {
        let list = list.clone();
        let status = status.clone();
        let refresh_action = refresh_button.clone();
        let fix_action = fix_button.clone();
        let install_action = install_button.clone();
        install_button.connect_clicked(move |_| {
            let buttons = [
                refresh_action.clone(),
                fix_action.clone(),
                install_action.clone(),
            ];
            for button in &buttons {
                button.set_sensitive(false);
            }
            set_status(
                &status,
                "در حال نصب بسته‌های پایه (ممکن است چند دقیقه طول بکشد)...",
                None,
            );

            let list = list.clone();
            let status = status.clone();
            run_background(
                || {
                    let outcome = install_base_packages(&SystemRunner, true);
                    let checks = run_diagnostics(&SystemRunner);
                    (outcome, checks)
                },
                move |(outcome, checks)| {
                    fill_diagnostics(&list, &checks);
                    match outcome {
                        Ok(message) => set_status(&status, &message, Some("success")),
                        Err(error) => set_status(&status, &error, Some("error")),
                    }
                    for button in &buttons {
                        button.set_sensitive(true);
                    }
                },
            );
        });
    }

    scroll_page(&page)
}

fn load_diagnostics(list: &gtk::ListBox, status: &gtk::Label) {
    set_status(status, "در حال بررسی وضعیت...", None);
    let list = list.clone();
    let status = status.clone();
    run_background(
        || run_diagnostics(&SystemRunner),
        move |checks| {
            fill_diagnostics(&list, &checks);
            show_diagnostics_summary(&status, &checks);
        },
    );
}

fn fill_diagnostics(list: &gtk::ListBox, checks: &[Check]) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    for check in checks {
        list.append(&check_row(check));
    }
}

fn show_diagnostics_summary(status: &gtk::Label, checks: &[Check]) {
    let errors = checks
        .iter()
        .filter(|check| check.state == CheckState::Error)
        .count();
    let warnings = checks
        .iter()
        .filter(|check| check.state == CheckState::Warning)
        .count();
    let infos = checks
        .iter()
        .filter(|check| check.state == CheckState::Info)
        .count();

    if errors == 0 && warnings == 0 {
        set_status(status, "همه بررسی‌ها موفق بود.", Some("success"));
        return;
    }

    let mut parts = Vec::new();
    if errors > 0 {
        parts.push(format!("{} خطا", persian_number(errors)));
    }
    if warnings > 0 {
        parts.push(format!("{} هشدار", persian_number(warnings)));
    }
    if infos > 0 {
        parts.push(format!("{} نکته", persian_number(infos)));
    }
    let kind = if errors > 0 { "error" } else { "warning" };
    set_status(
        status,
        &format!("بررسی کامل شد: {}", parts.join("، ")),
        Some(kind),
    );
}

fn check_row(check: &Check) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);

    let container = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    container.set_margin_top(8);
    container.set_margin_bottom(8);
    container.set_margin_start(8);
    container.set_margin_end(8);

    let icon = gtk::Image::from_icon_name(icon_for(check.state));
    icon.set_valign(gtk::Align::Start);

    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);

    let title = gtk::Label::builder()
        .label(&check.title)
        .halign(gtk::Align::Start)
        .wrap(true)
        .build();
    title.add_css_class("heading");
    text.append(&title);

    let detail = gtk::Label::builder()
        .label(&check.detail)
        .halign(gtk::Align::Start)
        .wrap(true)
        .build();
    detail.add_css_class("dim-label");
    text.append(&detail);

    if let Some(fix) = &check.fix {
        let fix_text = format!("راه‌حل: {fix}");
        let fix_label = gtk::Label::builder()
            .label(&fix_text)
            .halign(gtk::Align::Start)
            .wrap(true)
            .build();
        fix_label.add_css_class("caption");
        text.append(&fix_label);
    }

    container.append(&icon);
    container.append(&text);
    row.set_child(Some(&container));
    row
}

fn icon_for(state: CheckState) -> &'static str {
    match state {
        CheckState::Ok => "emblem-ok-symbolic",
        CheckState::Info => "dialog-information-symbolic",
        CheckState::Warning => "dialog-warning-symbolic",
        CheckState::Error => "dialog-error-symbolic",
    }
}

fn build_printer_page(state: &UiState) -> gtk::Widget {
    let (page, content) = titled_page(
        "۲. افزودن چاپگر",
        "چاپگر USB یا شبکه را انتخاب کنید. برای بیشتر چاپگرهای شبکه درایور خودکار (driverless) کافی است؛ برای USB مدرن بسته ipp-usb را نصب کنید.",
    );

    let search_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let search_button = gtk::Button::with_label("جستجوی چاپگر");
    let search_hint = gtk::Label::new(Some("کشف با lpinfo و Avahi انجام می‌شود."));
    search_hint.add_css_class("dim-label");
    search_hint.set_halign(gtk::Align::Start);
    search_hint.set_hexpand(true);
    search_row.append(&search_button);
    search_row.append(&search_hint);
    content.append(&search_row);

    let status = gtk::Label::new(None);
    status.set_halign(gtk::Align::Start);
    status.set_wrap(true);
    status.add_css_class("dim-label");
    content.append(&status);

    let discovered_title = gtk::Label::new(Some("چاپگرهای یافت‌شده"));
    discovered_title.add_css_class("heading");
    discovered_title.set_halign(gtk::Align::Start);
    content.append(&discovered_title);

    let printer_list = gtk::ListBox::new();
    printer_list.set_selection_mode(gtk::SelectionMode::Single);
    let printer_scroller = gtk::ScrolledWindow::builder()
        .min_content_height(200)
        .max_content_height(300)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&printer_list)
        .build();
    content.append(&printer_scroller);

    let recommendation =
        gtk::Label::new(Some("برای دیدن پیشنهاد درایور، یک چاپگر را انتخاب کنید."));
    recommendation.set_halign(gtk::Align::Start);
    recommendation.set_wrap(true);
    recommendation.add_css_class("dim-label");
    content.append(&recommendation);

    let form = gtk::Grid::builder()
        .column_spacing(8)
        .row_spacing(8)
        .build();
    let name_label = gtk::Label::builder()
        .label("نام صف:")
        .halign(gtk::Align::Start)
        .build();
    let name_entry = gtk::Entry::builder()
        .hexpand(true)
        .placeholder_text("مثلاً hp-officejet")
        .build();
    let driverless = gtk::CheckButton::with_label("درایور خودکار (driverless) — پیشنهادی");
    driverless.set_active(true);
    let model_label = gtk::Label::builder()
        .label("مدل درایور:")
        .halign(gtk::Align::Start)
        .build();
    let model_entry = gtk::Entry::builder()
        .hexpand(true)
        .placeholder_text("خروجی lpinfo -m (فقط حالت دستی)")
        .build();
    model_entry.set_sensitive(false);
    {
        let model_entry = model_entry.clone();
        driverless.connect_toggled(move |button| {
            model_entry.set_sensitive(!button.is_active());
        });
    }
    form.attach(&name_label, 0, 0, 1, 1);
    form.attach(&name_entry, 1, 0, 2, 1);
    form.attach(&driverless, 0, 1, 3, 1);
    form.attach(&model_label, 0, 2, 1, 1);
    form.attach(&model_entry, 1, 2, 2, 1);
    content.append(&form);

    let action_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let add_button = gtk::Button::with_label("افزودن چاپگر");
    let test_button = gtk::Button::with_label("چاپ آزمایشی");
    action_row.append(&add_button);
    action_row.append(&test_button);
    content.append(&action_row);

    let installed_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let installed_title = gtk::Label::new(Some("چاپگرهای نصب‌شده"));
    installed_title.add_css_class("heading");
    installed_title.set_halign(gtk::Align::Start);
    installed_title.set_hexpand(true);
    let installed_refresh = gtk::Button::with_label("به‌روزرسانی");
    installed_header.append(&installed_title);
    installed_header.append(&installed_refresh);
    content.append(&installed_header);

    let installed_list = gtk::ListBox::new();
    installed_list.set_selection_mode(gtk::SelectionMode::None);
    let installed_scroller = gtk::ScrolledWindow::builder()
        .min_content_height(90)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&installed_list)
        .build();
    content.append(&installed_scroller);

    refresh_installed(&installed_list, &status, || {});

    {
        let list = printer_list.clone();
        let state = state.clone();
        let name_entry = name_entry.clone();
        let status = status.clone();
        let search_widget = search_button.clone();
        search_button.connect_clicked(move |_| {
            search_widget.set_sensitive(false);
            set_status(&status, "در حال جستجوی چاپگرها...", None);

            let list = list.clone();
            let state = state.clone();
            let name_entry = name_entry.clone();
            let status = status.clone();
            let search_widget = search_widget.clone();
            run_background(
                || discover_printers(&SystemRunner),
                move |discovery| {
                    search_widget.set_sensitive(true);
                    while let Some(child) = list.first_child() {
                        list.remove(&child);
                    }
                    state.selected_printer.set(None);

                    let printers = discovery.printers;
                    for printer in &printers {
                        list.append(&printer_row(printer));
                    }
                    if let Some(first) = printers.first() {
                        name_entry.set_text(&first.queue_name);
                    }
                    let count = printers.len();
                    *state.printers.borrow_mut() = printers;

                    if count > 0 {
                        if let Some(row) = list.row_at_index(0) {
                            list.select_row(Some(&row));
                        }
                        let mut message = format!(
                            "{} چاپگر پیدا شد. یکی را انتخاب کنید.",
                            persian_number(count)
                        );
                        if !discovery.notes.is_empty() {
                            message.push_str(&format!(" {}", discovery.notes.join(" | ")));
                        }
                        set_status(&status, &message, Some("success"));
                    } else {
                        let mut message = "چاپگری پیدا نشد.".to_string();
                        if !discovery.notes.is_empty() {
                            message.push_str(&format!(" {}", discovery.notes.join(" | ")));
                        }
                        set_status(&status, &message, Some("warning"));
                    }
                },
            );
        });
    }

    {
        let state = state.clone();
        let name_entry = name_entry.clone();
        let model_entry = model_entry.clone();
        let driverless = driverless.clone();
        let recommendation = recommendation.clone();
        printer_list.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                if row.index() < 0 {
                    return;
                }
                let index = row.index() as usize;
                state.selected_printer.set(Some(index));
                if let Some(printer) = state.printers.borrow().get(index) {
                    name_entry.set_text(&printer.queue_name);
                    driverless.set_active(printer.driverless);
                    model_entry.set_sensitive(!printer.driverless);
                    if printer.driverless {
                        recommendation.set_text(&format!(
                            "درایور خودکار (IPP Everywhere) استفاده می‌شود. · {}",
                            printer.uri
                        ));
                    } else if let Some(suggestion) = recommend_print_package(&printer.detail) {
                        recommendation.set_text(&format!(
                            "بسته پیشنهادی: {} ({}). · {}",
                            suggestion.package, suggestion.reason, printer.uri
                        ));
                    } else {
                        recommendation.set_text(&format!(
                            "درایور پیشنهادی پیدا نشد؛ مدل را از lpinfo -m وارد کنید. · {}",
                            printer.uri
                        ));
                    }
                }
            } else {
                state.selected_printer.set(None);
                recommendation.set_text("برای دیدن پیشنهاد درایور، یک چاپگر را انتخاب کنید.");
            }
        });
    }

    {
        let state = state.clone();
        let name_entry = name_entry.clone();
        let model_entry = model_entry.clone();
        let driverless = driverless.clone();
        let installed_list = installed_list.clone();
        let status = status.clone();
        let add_widget = add_button.clone();
        add_button.connect_clicked(move |_| {
            let Some(index) = state.selected_printer.get() else {
                set_status(
                    &status,
                    "اول یک چاپگر را از فهرست انتخاب کنید.",
                    Some("warning"),
                );
                return;
            };
            let Some(printer) = state.printers.borrow().get(index).cloned() else {
                set_status(
                    &status,
                    "چاپگر انتخاب‌شده در دسترس نیست؛ دوباره جستجو کنید.",
                    Some("warning"),
                );
                return;
            };
            let queue = name_entry.text().trim().to_string();
            let manual_model = model_entry.text().trim().to_string();
            let model: Option<String> = if driverless.is_active() {
                None
            } else if manual_model.is_empty() {
                set_status(
                    &status,
                    "برای حالت دستی، مدل درایور را وارد کنید.",
                    Some("warning"),
                );
                return;
            } else {
                Some(manual_model)
            };

            add_widget.set_sensitive(false);
            set_status(&status, "در حال افزودن چاپگر...", None);

            let queue_message = queue.clone();
            let uri = printer.uri;
            let installed_list = installed_list.clone();
            let status = status.clone();
            let add_widget = add_widget.clone();
            run_background(
                move || {
                    let result =
                        add_printer(&SystemRunner, &queue, &uri, model.as_deref(), true, true);
                    let queues = list_queues(&SystemRunner);
                    (result, queues)
                },
                move |(result, queues)| {
                    render_installed(&installed_list, &status, queues);
                    match result {
                        Ok(_) => set_status(
                            &status,
                            &format!("چاپگر «{queue_message}» اضافه و پیش‌فرض شد."),
                            Some("success"),
                        ),
                        Err(error) => set_status(&status, &error, Some("error")),
                    }
                    add_widget.set_sensitive(true);
                },
            );
        });
    }

    {
        let name_entry = name_entry.clone();
        let status = status.clone();
        let test_widget = test_button.clone();
        test_button.connect_clicked(move |_| {
            let queue = name_entry.text().trim().to_string();
            if queue.is_empty() {
                set_status(
                    &status,
                    "نام صف را وارد کنید یا یک چاپگر نصب‌شده را انتخاب کنید.",
                    Some("warning"),
                );
                return;
            }
            test_print_async(&test_widget, &status, queue);
        });
    }

    {
        let installed_list = installed_list.clone();
        let status = status.clone();
        let refresh_widget = installed_refresh.clone();
        installed_refresh.connect_clicked(move |_| {
            refresh_widget.set_sensitive(false);
            let button = refresh_widget.clone();
            refresh_installed(&installed_list, &status, move || {
                button.set_sensitive(true);
            });
        });
    }

    scroll_page(&page)
}

fn printer_row(printer: &DiscoveredPrinter) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    let container = gtk::Box::new(gtk::Orientation::Vertical, 2);
    container.set_margin_top(8);
    container.set_margin_bottom(8);
    container.set_margin_start(8);
    container.set_margin_end(8);

    let title = gtk::Label::builder()
        .label(&printer.name)
        .halign(gtk::Align::Start)
        .wrap(true)
        .build();
    title.add_css_class("heading");
    container.append(&title);

    let transport = match printer.transport {
        PrinterTransport::Usb => "USB",
        PrinterTransport::Network => "شبکه",
        PrinterTransport::Other => "سایر",
    };
    let origin = match printer.origin {
        PrinterOrigin::CupsDevices => "CUPS",
        PrinterOrigin::Avahi => "Avahi",
    };
    let mode = if printer.driverless {
        "driverless"
    } else {
        "درایور دستی"
    };
    let detail_text = format!("{transport} · {origin} · {mode}");
    let detail = gtk::Label::builder()
        .label(&detail_text)
        .halign(gtk::Align::Start)
        .wrap(true)
        .build();
    detail.add_css_class("dim-label");
    container.append(&detail);

    row.set_child(Some(&container));
    row
}

fn refresh_installed<F>(list: &gtk::ListBox, status: &gtk::Label, done: F)
where
    F: FnOnce() + 'static,
{
    let list = list.clone();
    let status = status.clone();
    run_background(
        || list_queues(&SystemRunner),
        move |result| {
            render_installed(&list, &status, result);
            done();
        },
    );
}

fn render_installed(
    list: &gtk::ListBox,
    status: &gtk::Label,
    result: Result<Vec<InstalledPrinter>, String>,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    match result {
        Ok(queues) if queues.is_empty() => {
            let row = gtk::ListBoxRow::new();
            row.set_activatable(false);
            let label = gtk::Label::builder()
                .label("هنوز چاپگری نصب نشده است.")
                .halign(gtk::Align::Start)
                .build();
            label.add_css_class("dim-label");
            label.set_margin_top(8);
            label.set_margin_bottom(8);
            label.set_margin_start(8);
            label.set_margin_end(8);
            row.set_child(Some(&label));
            list.append(&row);
        }
        Ok(queues) => {
            for queue in queues {
                let row = gtk::ListBoxRow::new();
                row.set_activatable(false);

                let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                line.set_margin_top(6);
                line.set_margin_bottom(6);
                line.set_margin_start(8);
                line.set_margin_end(8);

                let name = gtk::Label::builder()
                    .label(&queue.name)
                    .halign(gtk::Align::Start)
                    .hexpand(true)
                    .build();
                name.add_css_class("heading");

                let state_text = if queue.is_default {
                    format!("{} — پیش‌فرض", queue.state)
                } else {
                    queue.state.clone()
                };
                let queue_state = gtk::Label::builder()
                    .label(&state_text)
                    .halign(gtk::Align::Start)
                    .wrap(true)
                    .build();
                queue_state.add_css_class("dim-label");

                let test = gtk::Button::with_label("چاپ آزمایشی");
                {
                    let queue_name = queue.name.clone();
                    let status = status.clone();
                    let test_widget = test.clone();
                    test.connect_clicked(move |_| {
                        test_print_async(&test_widget, &status, queue_name.clone());
                    });
                }

                line.append(&name);
                line.append(&queue_state);
                line.append(&test);
                row.set_child(Some(&line));
                list.append(&row);
            }
        }
        Err(error) => {
            let row = gtk::ListBoxRow::new();
            row.set_activatable(false);
            let label = gtk::Label::builder()
                .label(&error)
                .halign(gtk::Align::Start)
                .wrap(true)
                .build();
            label.add_css_class("error");
            label.set_margin_top(8);
            label.set_margin_bottom(8);
            label.set_margin_start(8);
            label.set_margin_end(8);
            row.set_child(Some(&label));
            list.append(&row);
        }
    }
}

fn test_print_async(button: &gtk::Button, status: &gtk::Label, queue: String) {
    button.set_sensitive(false);
    set_status(
        status,
        &format!("در حال ارسال صفحه آزمایشی به «{queue}»..."),
        None,
    );

    let button = button.clone();
    let status = status.clone();
    run_background(
        move || test_print(&SystemRunner, &queue),
        move |result| {
            button.set_sensitive(true);
            match result {
                Ok(message) => set_status(&status, &message, Some("success")),
                Err(error) => set_status(&status, &error, Some("error")),
            }
        },
    );
}

fn build_scanner_page(state: &UiState) -> gtk::Widget {
    let (page, content) = titled_page(
        "۳. اسکنر",
        "اسکنرهای USB و شبکه با SANE شناسایی می‌شوند. برای اسکنرهای شبکه eSCL نصب sane-airscan توصیه می‌شود.",
    );

    let search_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let search_button = gtk::Button::with_label("جستجوی اسکنر");
    let search_hint = gtk::Label::new(Some("شناسایی با scanimage -L انجام می‌شود."));
    search_hint.add_css_class("dim-label");
    search_hint.set_halign(gtk::Align::Start);
    search_hint.set_hexpand(true);
    search_row.append(&search_button);
    search_row.append(&search_hint);
    content.append(&search_row);

    let scanner_title = gtk::Label::new(Some("اسکنرهای یافت‌شده"));
    scanner_title.add_css_class("heading");
    scanner_title.set_halign(gtk::Align::Start);
    content.append(&scanner_title);

    let scanner_list = gtk::ListBox::new();
    scanner_list.set_selection_mode(gtk::SelectionMode::Single);
    let scanner_scroller = gtk::ScrolledWindow::builder()
        .min_content_height(100)
        .max_content_height(280)
        .propagate_natural_height(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&scanner_list)
        .build();
    content.append(&scanner_scroller);

    let recommendation =
        gtk::Label::new(Some("برای دیدن پیشنهاد درایور، یک اسکنر را انتخاب کنید."));
    recommendation.set_halign(gtk::Align::Start);
    recommendation.set_wrap(true);
    recommendation.add_css_class("dim-label");
    content.append(&recommendation);

    let resolution_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let resolution_label = gtk::Label::new(Some("رزولوشن اسکن:"));
    resolution_label.add_css_class("dim-label");
    let resolution = gtk::DropDown::from_strings(&["۷۵ dpi", "۱۵۰ dpi", "۳۰۰ dpi", "۶۰۰ dpi"]);
    resolution.set_selected(1);
    resolution_row.append(&resolution_label);
    resolution_row.append(&resolution);
    content.append(&resolution_row);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let install_button = gtk::Button::with_label("نصب بسته پیشنهادی");
    install_button.set_sensitive(false);
    let scan_button = gtk::Button::with_label("اسکن آزمایشی");
    scan_button.set_sensitive(false);
    let open_button = gtk::Button::with_label("نمایش فایل");
    open_button.set_sensitive(false);
    actions.append(&install_button);
    actions.append(&scan_button);
    actions.append(&open_button);
    content.append(&actions);

    let status = gtk::Label::new(None);
    status.set_halign(gtk::Align::Start);
    status.set_wrap(true);
    status.add_css_class("dim-label");
    content.append(&status);

    {
        let list = scanner_list.clone();
        let state = state.clone();
        let status = status.clone();
        let search_widget = search_button.clone();
        search_button.connect_clicked(move |_| {
            search_widget.set_sensitive(false);
            set_status(&status, "در حال جستجوی اسکنرها...", None);

            let list = list.clone();
            let state = state.clone();
            let status = status.clone();
            let search_widget = search_widget.clone();
            run_background(
                || discover_scanners(&SystemRunner),
                move |discovery| {
                    search_widget.set_sensitive(true);
                    while let Some(child) = list.first_child() {
                        list.remove(&child);
                    }
                    state.selected_scanner.set(None);

                    let scanners = discovery.scanners;
                    for scanner in &scanners {
                        list.append(&scanner_row(scanner));
                    }
                    let count = scanners.len();
                    *state.scanners.borrow_mut() = scanners;

                    if count > 0 {
                        if let Some(row) = list.row_at_index(0) {
                            list.select_row(Some(&row));
                        }
                        let mut message = format!("{} اسکنر پیدا شد.", persian_number(count));
                        if !discovery.notes.is_empty() {
                            message.push_str(&format!(" {}", discovery.notes.join(" | ")));
                        }
                        if !discovery.usb_findings.is_empty() {
                            message
                                .push_str(&format!(" USB: {}", discovery.usb_findings.join(" | ")));
                        }
                        set_status(&status, &message, Some("success"));
                    } else {
                        let mut message = "اسکنری پیدا نشد.".to_string();
                        if !discovery.notes.is_empty() {
                            message.push_str(&format!(" {}", discovery.notes.join(" | ")));
                        }
                        set_status(&status, &message, Some("warning"));
                    }
                },
            );
        });
    }

    {
        let state = state.clone();
        let recommendation = recommendation.clone();
        let install_button = install_button.clone();
        let scan_button = scan_button.clone();
        scanner_list.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                if row.index() < 0 {
                    return;
                }
                let index = row.index() as usize;
                state.selected_scanner.set(Some(index));
                if let Some(scanner) = state.scanners.borrow().get(index) {
                    match recommend_scanner_package(&scanner.description, &scanner.device) {
                        Some(recommendation_info) => {
                            let text = format!(
                                "بسته پیشنهادی: {} — {}",
                                recommendation_info.package, recommendation_info.reason
                            );
                            recommendation.set_text(&text);
                            install_button.set_sensitive(true);
                        }
                        None => {
                            recommendation.set_text(
                                "بسته درایور اختصاصی پیشنهاد نشد. برای اسکنر شبکه، sane-airscan را بررسی کنید.",
                            );
                            install_button.set_sensitive(false);
                        }
                    }
                    scan_button.set_sensitive(true);
                }
            } else {
                state.selected_scanner.set(None);
                install_button.set_sensitive(false);
                scan_button.set_sensitive(false);
            }
        });
    }

    {
        let state = state.clone();
        let status = status.clone();
        let install_widget = install_button.clone();
        install_button.connect_clicked(move |_| {
            let Some(index) = state.selected_scanner.get() else {
                set_status(
                    &status,
                    "اول یک اسکنر را از فهرست انتخاب کنید.",
                    Some("warning"),
                );
                return;
            };
            let Some(scanner) = state.scanners.borrow().get(index).cloned() else {
                set_status(
                    &status,
                    "اسکنر انتخاب‌شده در دسترس نیست؛ دوباره جستجو کنید.",
                    Some("warning"),
                );
                return;
            };
            let Some(recommendation_info) =
                recommend_scanner_package(&scanner.description, &scanner.device)
            else {
                set_status(
                    &status,
                    "برای این دستگاه بسته‌ای پیشنهاد نشده است.",
                    Some("warning"),
                );
                return;
            };
            if recommendation_info.official {
                install_widget.set_sensitive(false);
                set_status(&status, "در حال نصب بسته پیشنهادی...", None);

                let package = recommendation_info.package.to_string();
                let status = status.clone();
                let state = state.clone();
                let install_widget = install_widget.clone();
                run_background(
                    move || install_packages(&SystemRunner, &[package], true),
                    move |result| {
                        install_widget.set_sensitive(state.selected_scanner.get().is_some());
                        match result {
                            Ok(message) => set_status(&status, &message, Some("success")),
                            Err(error) => set_status(&status, &error, Some("error")),
                        }
                    },
                );
            } else {
                let helper = aur_helper(&SystemRunner).unwrap_or("paru");
                set_status(
                    &status,
                    &format!(
                        "بسته {} در AUR است. با دستور «{} -S {}» نصب کنید.",
                        recommendation_info.package, helper, recommendation_info.package
                    ),
                    Some("warning"),
                );
            }
        });
    }

    {
        let state = state.clone();
        let status = status.clone();
        let open_button = open_button.clone();
        let scan_widget = scan_button.clone();
        scan_button.connect_clicked(move |_| {
            let Some(index) = state.selected_scanner.get() else {
                set_status(
                    &status,
                    "اول یک اسکنر را از فهرست انتخاب کنید.",
                    Some("warning"),
                );
                return;
            };
            let Some(scanner) = state.scanners.borrow().get(index).cloned() else {
                set_status(
                    &status,
                    "اسکنر انتخاب‌شده در دسترس نیست؛ دوباره جستجو کنید.",
                    Some("warning"),
                );
                return;
            };
            let output = default_scan_path();
            let resolution = match resolution.selected() {
                0 => 75,
                2 => 300,
                3 => 600,
                _ => 150,
            };
            scan_widget.set_sensitive(false);
            set_status(&status, "در حال اسکن آزمایشی...", None);

            let output_message = output.clone();
            let device = scanner.device;
            let status = status.clone();
            let scan_widget = scan_widget.clone();
            let open_button = open_button.clone();
            let state = state.clone();
            run_background(
                move || run_test_scan(&SystemRunner, &device, &output, resolution),
                move |result| {
                    scan_widget.set_sensitive(state.selected_scanner.get().is_some());
                    match result {
                        Ok(message) => {
                            *state.scan_output.borrow_mut() = Some(output_message.clone());
                            open_button.set_sensitive(true);
                            set_status(
                                &status,
                                &format!("{message} فایل: {}", output_message.display()),
                                Some("success"),
                            );
                        }
                        Err(error) => set_status(&status, &error, Some("error")),
                    }
                },
            );
        });
    }

    {
        let state = state.clone();
        let status = status.clone();
        open_button.connect_clicked(move |_| {
            let path = state.scan_output.borrow().clone();
            match path {
                Some(path) => {
                    if let Err(error) = ProcessCommand::new("xdg-open").arg(&path).spawn() {
                        set_status(
                            &status,
                            &format!("باز کردن فایل ناموفق بود: {error}"),
                            Some("error"),
                        );
                    }
                }
                None => set_status(&status, "هنوز فایلی اسکن نشده است.", Some("warning")),
            }
        });
    }

    scroll_page(&page)
}

fn scanner_row(scanner: &Scanner) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    let container = gtk::Box::new(gtk::Orientation::Vertical, 2);
    container.set_margin_top(8);
    container.set_margin_bottom(8);
    container.set_margin_start(8);
    container.set_margin_end(8);

    let title = gtk::Label::builder()
        .label(&scanner.description)
        .halign(gtk::Align::Start)
        .wrap(true)
        .build();
    title.add_css_class("heading");
    container.append(&title);

    let kind = if scanner.network {
        "شبکه"
    } else {
        "محلی"
    };
    let detail_text = format!("{kind} · {}", scanner.device);
    let detail = gtk::Label::builder()
        .label(&detail_text)
        .halign(gtk::Align::Start)
        .wrap(true)
        .build();
    detail.add_css_class("dim-label");
    container.append(&detail);

    row.set_child(Some(&container));
    row
}

fn build_finish_page() -> gtk::Widget {
    let (page, content) = titled_page(
        "۴. پایان",
        "اگر چاپ آزمایشی و اسکن آزمایشی موفق بود، تنظیمات کامل است.",
    );

    let tips_heading = gtk::Label::builder()
        .label("نکته‌ها:")
        .halign(gtk::Align::Start)
        .build();
    content.append(&tips_heading);

    let tips = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let tip_items = [
        "مدیریت پیشرفته چاپگرها: http://localhost:631",
        "اگر چاپگر پیدا نشد، سرویس cups.socket و باز بودن UDP 5353 برای mDNS را بررسی کنید.",
        "اگر اسکنر دسترسی نداشت، عضو گروه‌های scanner و lp شوید و دوباره وارد شوید.",
        "درایورهای AUR را با paru یا yay نصب کنید؛ ویزارد آن‌ها را خودکار نصب نمی‌کند.",
        "گزارش کامل وضعیت با دستور linuprint diagnose نمایش داده می‌شود.",
    ];
    for tip in tip_items {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let bullet = gtk::Label::new(Some("•"));
        bullet.add_css_class("dim-label");
        bullet.set_valign(gtk::Align::Start);
        let text = gtk::Label::builder()
            .label(tip)
            .halign(gtk::Align::Start)
            .wrap(true)
            .hexpand(true)
            .build();
        row.append(&bullet);
        row.append(&text);
        tips.append(&row);
    }
    content.append(&tips);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let cups_button = gtk::Button::with_label("باز کردن CUPS");
    actions.append(&cups_button);
    content.append(&actions);

    let status = gtk::Label::new(None);
    status.set_halign(gtk::Align::Start);
    status.set_wrap(true);
    status.add_css_class("dim-label");
    content.append(&status);

    {
        let status = status.clone();
        cups_button.connect_clicked(move |_| {
            if let Err(error) = ProcessCommand::new("xdg-open")
                .arg("http://localhost:631")
                .spawn()
            {
                set_status(
                    &status,
                    &format!("باز کردن مرورگر ناموفق بود: {error}"),
                    Some("error"),
                );
            }
        });
    }

    scroll_page(&page)
}

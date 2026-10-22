# LinuPrint

[![Release](https://github.com/TheGreatAzizi/LinuPrint/actions/workflows/release.yml/badge.svg)](https://github.com/TheGreatAzizi/LinuPrint/actions/workflows/release.yml)

ویزاردی گام‌به‌گام و بومی GTK4 برای راه‌اندازی چاپگر و اسکنر در آرچ لینوکس و توزیع‌های آرچ‌بیس. این برنامه به‌جای پنهان‌کردن جزئیات، از همان ابزارهای استاندارد سیستم استفاده می‌کند: **CUPS**، **Avahi**، **SANE** و **pacman**.

## امکانات

- **بررسی پیش‌نیازها**: نصب‌بودن بسته‌های پایه، وضعیت `cups.socket` و `avahi-daemon`، پیکربندی `nsswitch.conf`، دسترسی مدیریتی و گروه‌های کاربر
- **کشف چاپگر**: دستگاه‌های USB با `lpinfo -v` و چاپگرهای شبکه با Avahi (IPP/IPPS/PDL)
- **افزودن چاپگر**: حالت driverless (`everywhere`) یا انتخاب مدل دستی، تنظیم به‌عنوان چاپگر پیش‌فرض و فعال‌سازی صف
- **چاپ آزمایشی**: ارسال یک صفحه تست به صف انتخاب‌شده
- **کشف اسکنر**: شناسایی اسکنرهای USB و شبکه با `scanimage -L` و بررسی `sane-find-scanner` در صورت نیاز
- **اسکن آزمایشی**: خروجی PNG با رزولوشن قابل انتخاب
- **پیشنهاد درایور**: نگاشت خودکار برند به بسته‌های رسمی و AUR
- **رفع سریع**: نصب بسته‌های پایه و فعال‌سازی سرویس‌ها با `pkexec`
- **CLI کامل**: برای اسکریپت‌نویسی و عیب‌یابی بدون رابط گرافیکی

## پیش‌نیازها

- آرچ لینوکس یا توزیع آرچ‌بیس با systemd
- Rust 1.92 یا جدیدتر
- GTK 4 (فقط برای رابط گرافیکی)
- بسته‌های زمان اجرا: `cups`، `avahi`، `sane`، `polkit`، `xdg-utils`
- بسته‌های پیشنهادی: `cups-filters`، `nss-mdns`، `sane-airscan`، `ipp-usb` (چاپ driverless روی USB)، درایور برند چاپگر

```bash
sudo pacman -S --needed base-devel rust git gtk4 pkgconf cups cups-filters avahi nss-mdns sane sane-airscan polkit xdg-utils
```

> برای چاپگرهای HP بسته `hplip`، برای اپسون `epson-inkjet-printer-escpr` و برای برادر `brlaser` را نصب کنید. برنامه در صورت نیاز پیشنهاد می‌دهد.

## ساخت و اجرا

```bash
git clone https://github.com/TheGreatAzizi/LinuPrint.git
cd LinuPrint

# فقط CLI (بدون وابستگی GTK در زمان ساخت)
cargo build --release

# ساخت رابط گرافیکی
cargo build --release --features gui

# اجرای ویزارد
./target/release/linuprint gui

# نصب در سیستم
sudo install -Dm755 target/release/linuprint /usr/local/bin/linuprint
```

اگر GTK نصب نباشد نسخه CLI همچنان کار می‌کند. برای ساخت کامل ویزارد، ویژگی `gui` را فعال کنید.

## دستورهای CLI

```bash
# گزارش وضعیت و پیشنهاد راه‌حل
linuprint diagnose

# فعال‌سازی سرویس‌های لازم
linuprint services

# فهرست چاپگرهای کشف‌شده و نصب‌شده
linuprint printers list

# افزودن چاپگر driverless و تنظیم به‌عنوان پیش‌فرض
linuprint printers add \
  --uri ipp://192.168.1.20:631/ipp/print \
  --name hp-officejet

# افزودن با مدل درایور مشخص
linuprint printers add \
  --uri usb://HP/LaserJet \
  --name hp-laserjet \
  --model drv:///hp/hpcups.drv/hp-laserjet.drv

# چاپ آزمایشی و حذف صف
linuprint printers test hp-officejet
linuprint printers remove hp-officejet

# فهرست اسکنرها
linuprint scanners list

# اسکن آزمایشی
linuprint scanners scan \
  --device "escl:https://192.168.1.20:443" \
  --output ~/scan-test.png \
  --resolution 150

# نصب بسته‌های رسمی
linuprint install sane-airscan
```

## ساختار پروژه

```text
src/
  backend/
    shell.rs        اجرای امن فرمان‌ها و مدیریت دسترسی (pkexec/sudo)
    printers.rs     کشف، افزودن، حذف و چاپ آزمایشی
    avahi.rs        تجزیه خروجی avahi-browse و ساخت URI
    scanners.rs     کشف و اسکن با SANE
    diagnostics.rs  بررسی وضعیت و گزارش
    drivers.rs      پیشنهاد بسته درایور
    packages.rs     بررسی/نصب بسته‌ها و فعال‌سازی سرویس‌ها
  cli.rs            رابط خط فرمان
  gui.rs            ویزارد GTK4 (ویژگی gui)
  lib.rs
  main.rs
tests/
  parsing.rs        تجزیه خروجی ابزارها
  commands.rs       ساخت فرمان‌ها و مدیریت دسترسی
  diagnostics.rs    سناریوهای سالم/خراب با Runner جعلی
packaging/
  PKGBUILD
  .SRCINFO
  linuprint.desktop
  linuprint.svg
.github/
  workflows/release.yml
  scripts/build-arch-package.sh
  scripts/build-in-arch.sh
```

## نحوه کار

- فرمان‌ها بدون shell و به‌صورت آرگومان‌های جدا اجرا می‌شوند؛ ورودی کاربر هرگز به رشته shell تبدیل نمی‌شود.
- عملیات مدیریتی با `pkexec` (در GUI) یا `sudo` (در CLI) انجام می‌شود.
- نام صف قبل از استفاده اعتبارسنجی می‌شود و URI/مدل نمی‌توانند با `-` شروع شوند.
- نصب بسته‌های AUR خودکار نیست؛ برنامه فقط بسته و فرمان پیشنهادی را نمایش می‌دهد.
- کشف چاپگر با `lpinfo` و Avahi انجام می‌شود و نتایج تکراری ادغام می‌شوند.
- برای چاپگرهای مدرن، `-m everywhere` (IPP Everywhere) پیش‌فرض است.

## عیب‌یابی

| مشکل | راه‌حل |
| --- | --- |
| چاپگر شبکه پیدا نمی‌شود | `systemctl enable --now avahi-daemon.service`، باز بودن UDP 5353 و نصب `nss-mdns` |
| نام‌های `.local` حل نمی‌شوند | افزودن `mdns_minimal` به خط `hosts` در `/etc/nsswitch.conf` |
| افزودن چاپگر شکست می‌خورد | `systemctl status cups.socket` و `journalctl -u cups` را بررسی کنید |
| چاپگر USB driverless کار نمی‌کند | برای چاپ driverless، بسته `ipp-usb` را نصب و دستگاه را دوباره وصل کنید؛ در غیر این صورت تیک driverless را بردارید و مدل را از `lpinfo -m` وارد کنید |
| اسکنر پیدا نمی‌شود | نصب `sane` و `sane-airscan`، سپس `scanimage -L` |
| اسکنر دسترسی ندارد | عضویت در گروه‌های `scanner` و `lp` و ورود مجدد |
| خطای `No authentication agent found` | از یک نشست دسکتاپ با عامل polkit استفاده کنید یا عملیات را با CLI و `sudo` انجام دهید |
| باز شدن فایل یا CUPS ناموفق است | بسته `xdg-utils` را نصب کنید |
| درایور فقط در AUR است | با `paru -S <package>` یا `yay -S <package>` نصب کنید |

گزارش کامل:

```bash
linuprint diagnose
```

## بسته‌بندی و انتشار

`PKGBUILD` مستقیماً از تگ همین مخزن می‌سازد و `.SRCINFO` در مخزن نگهداری می‌شود. برای ساخت محلی:

```bash
cd packaging
makepkg -si
```

> `pkgver` در `PKGBUILD` و `Cargo.toml` باید با تگ `vX.Y.Z` هم‌خوان باشد. پس از هر تغییر در `PKGBUILD`، فایل `.SRCINFO` را دوباره بسازید:
>
> ```bash
> cd packaging
> makepkg --printsrcinfo > .SRCINFO
> ```

### انتشار خودکار با GitHub Actions

با push یک تگ نسخه، گردش‌کار `Release` اجرا می‌شود:

```bash
git tag v0.1.0
git push origin v0.1.0
```

این گردش‌کار داخل کانتینر Arch بسته را با `makepkg` می‌سازد و فایل‌های `linuprint-0.1.0-1-x86_64.pkg.tar.zst` و `SHA256SUMS` را به release همان تگ اضافه می‌کند.

برای ساخت تگ از خود GitHub هم می‌توانید از مسیر **Actions → Release → Run workflow** و وارد کردن نسخه (مثلاً `v0.1.0`) استفاده کنید.

## تست

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

برای بررسی رابط گرافیکی روی آرچ:

```bash
cargo clippy --all-targets --features gui -- -D warnings
```

## امنیت

- هیچ فرمانی از طریق shell اجرا نمی‌شود؛ بنابراین تزریق فرمان ممکن نیست.
- نصب بسته و تغییر سرویس فقط با تأیید صریح کاربر و از طریق polkit/sudo انجام می‌شود.
- بسته‌های AUR به‌صورت خودکار نصب نمی‌شوند.
- صفحه تست در پوشه موقت ساخته و بعد از ارسال حذف می‌شود.
- تنها خواندن `/etc/nsswitch.conf` برای بررسی mDNS انجام می‌شود؛ تغییرات سیستم فقط از طریق ابزارهای رسمی CUPS/SANE/systemd و با تأیید polkit/sudo اعمال می‌شوند.

## مجوز

MIT

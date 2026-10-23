<p align="center">
  <img src="packaging/linuprint.svg" width="96" height="96" alt="LinuPrint">
</p>

<h1 align="center">LinuPrint</h1>

<p align="center">
  <strong>راه‌اندازی چاپگر و اسکنر در آرچ لینوکس، ساده و شفاف</strong>
</p>

<p align="center">
  <a href="https://github.com/TheGreatAzizi/LinuPrint/releases"><img alt="Release" src="https://img.shields.io/github/v/release/TheGreatAzizi/LinuPrint?style=flat-square&label=release"></a>
  <a href="https://github.com/TheGreatAzizi/LinuPrint/blob/main/LICENSE"><img alt="License" src="https://img.shields.io/github/license/TheGreatAzizi/LinuPrint?style=flat-square"></a>
  <a href="https://archlinux.org/"><img alt="Arch Linux" src="https://img.shields.io/badge/platform-Arch%20Linux-1793d1?style=flat-square&logo=archlinux&logoColor=white"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/rust-1.92%2B-dea584?style=flat-square&logo=rust&logoColor=white"></a>
</p>

<p align="center">
  <a href="https://github.com/TheGreatAzizi/LinuPrint">مخزن GitHub</a>
  ·
  <a href="https://github.com/TheGreatAzizi/LinuPrint/releases">دانلود آخرین نسخه</a>
  ·
  <a href="https://github.com/TheGreatAzizi/LinuPrint/issues">گزارش مشکل</a>
</p>

LinuPrint یک ویزارد گام‌به‌گام و بومی GTK4 برای راه‌اندازی چاپگر و اسکنر در آرچ لینوکس و توزیع‌های آرچ‌بیس است. این برنامه جزئیات را پنهان نمی‌کند و از همان ابزارهای استاندارد سیستم استفاده می‌کند: **CUPS**، **Avahi**، **SANE** و **pacman**.

> **LinuPrint** is a step-by-step GTK4 wizard for setting up printers and scanners on Arch Linux. It uses the standard system tools directly, keeps privileged operations explicit, and gives users a clear view of what is happening.

## فهرست

- [LinuPrint چیست؟](#linuprint-چیست)
- [امکانات](#امکانات)
- [پیش‌نیازها](#پیشنیازها)
- [نصب](#نصب)
- [شروع سریع](#شروع-سریع)
- [رابط گرافیکی](#رابط-گرافیکی)
- [مرجع CLI](#مرجع-cli)
- [نحوه کار و امنیت](#نحوه-کار-و-امنیت)
- [عیب‌یابی](#عیبیابی)
- [توسعه](#توسعه)
- [بسته‌بندی و انتشار](#بستهبندی-و-انتشار)
- [مشارکت](#مشارکت)
- [مجوز](#مجوز)
- [English summary](#english-summary)

## LinuPrint چیست؟

بسیاری از کاربران آرچ برای اولین بار با چند پرسش روبه‌رو می‌شوند: چاپگر USB را چطور به CUPS معرفی کنم؟ برای چاپگر شبکه به Avahi یا `nss-mdns` نیاز دارم؟ درایور درست کدام است؟ چرا کار در صف مانده است؟ LinuPrint این مسیر را به یک جریان روشن تبدیل می‌کند:

1. وضعیت سیستم و پیش‌نیازها را بررسی می‌کند.
2. چاپگرهای USB و شبکه را کشف می‌کند.
3. صف CUPS را با driverless یا PPD مناسب می‌سازد.
4. صفحه آزمون چاپ می‌کند و امکان چاپ فایل و مدیریت صف را می‌دهد.
5. اسکنرهای SANE را پیدا می‌کند و اسکن را در قالب PNG، JPEG یا PDF ذخیره می‌کند.

LinuPrint جایگزین CUPS یا SANE نیست؛ یک لایه راهنما و خودکارسازی روی همان ابزارهای رسمی است. اگر چیزی درست کار نکرد، فرمان‌ها و گزارش‌های قابل بررسی در اختیار شما هستند.

## امکانات

### چاپ

- **کشف چاپگر**: دستگاه‌های USB با `lpinfo -v` و چاپگرهای شبکه با Avahi (IPP/IPPS/PDL).
- **افزودن خودکار**: تشخیص driverless برای چاپگرهای IPP، نصب بسته درایور رسمی در صورت نیاز، انتخاب بهترین PPD از `lpinfo -m`، تنظیم چاپگر پیش‌فرض و فعال‌سازی صف.
- **چاپ آزمایشی**: تولید یک صفحه آزمون PDF شامل معرفی LinuPrint، نام صف، نسخه برنامه و راهنمای امکانات؛ سپس ارسال آن با `lp`.
- **چاپ فایل**: چاپ PDF، تصویر یا متن با تعداد نسخه، محدوده صفحه، چاپ دوطرفه، تناسب با صفحه و اندازه کاغذ.
- **مدیریت صف**: مشاهده کارهای در انتظار، لغو یک کار یا همه کارهای یک صف، تنظیم چاپگر پیش‌فرض و توقف/ادامه صف.

### اسکن

- **کشف اسکنر**: شناسایی اسکنرهای USB و شبکه با `scanimage -L` و بررسی `sane-find-scanner` در صورت نیاز.
- **قالب خروجی**: PNG، JPEG و PDF.
- **حالت رنگ**: خودکار، رنگی، خاکستری و سیاه‌وسفید.
- **رزولوشن قابل انتخاب**: از ۷۵ تا ۶۰۰ DPI در رابط گرافیکی.
- **ذخیره‌سازی**: ذخیره در `~/Documents` با نام زمان‌دار، یا انتخاب مسیر دلخواه در GUI و CLI.

### سیستم و رابط

- **بررسی پیش‌نیازها**: نصب‌بودن بسته‌های پایه، وضعیت `cups.socket` و `avahi-daemon`، پیکربندی `nsswitch.conf`، دسترسی مدیریتی و گروه‌های کاربر.
- **رفع سریع**: نصب بسته‌های پایه و فعال‌سازی سرویس‌ها با `pkexec`.
- **رابط فارسی و راست‌به‌چپ**: ویزارد پنج‌مرحله‌ای GTK4 با بازخورد وضعیت در هر مرحله.
- **دکمه GitHub**: باز کردن مخزن پروژه در مرورگر پیش‌فرض از نوار بالای برنامه.
- **CLI کامل**: برای اسکریپت‌نویسی و عیب‌یابی بدون رابط گرافیکی.

## پیش‌نیازها

| مورد | حداقل | توضیح |
| --- | --- | --- |
| سیستم‌عامل | Arch Linux یا Arch-based | مبتنی بر systemd و pacman |
| Rust | 1.92 | فقط برای ساخت از منبع |
| GTK | 4.10 | فقط برای رابط گرافیکی |
| CUPS | بسته `cups` | چاپ و مدیریت صف |
| Avahi | `avahi` و `nss-mdns` | کشف چاپگرهای شبکه |
| SANE | `sane` و `sane-airscan` | اسکنرهای USB و شبکه |
| Polkit | `polkit` | اجرای عملیات مدیریتی در GUI |
| xdg-utils | `xdg-utils` | باز کردن مرورگر و CUPS |

بسته‌های پیشنهادی: `cups-filters`، `ipp-usb` (چاپ driverless روی USB) و درایور رسمی برند چاپگر مانند `hplip` برای HP، `epson-inkjet-printer-escpr` برای Epson و `brlaser` برای Brother.

```bash
sudo pacman -S --needed base-devel rust git gtk4 pkgconf \
  cups cups-filters avahi nss-mdns sane sane-airscan polkit xdg-utils
```

> LinuPrint بسته‌های رسمی درایور را در صورت نیاز هنگام افزودن چاپگر نصب می‌کند. بسته‌های AUR به‌صورت خودکار نصب نمی‌شوند و فقط فرمان پیشنهادی نمایش داده می‌شود.

## نصب

### بسته آماده

بسته Arch را از [صفحه Releases](https://github.com/TheGreatAzizi/LinuPrint/releases) دانلود کنید و همراه با فایل `SHA256SUMS` بررسی و نصب کنید:

```bash
sha256sum -c SHA256SUMS
sudo pacman -U ./linuprint-*-x86_64.pkg.tar.zst
```

### ساخت از منبع

```bash
git clone https://github.com/TheGreatAzizi/LinuPrint.git
cd LinuPrint
cargo build --release --features gui
sudo install -Dm755 target/release/linuprint /usr/local/bin/linuprint
sudo install -Dm644 packaging/linuprint.desktop /usr/share/applications/linuprint.desktop
sudo install -Dm644 packaging/linuprint.svg /usr/share/icons/hicolor/scalable/apps/linuprint.svg
```

### فقط CLI

اگر GTK نصب نیست، نسخه خط فرمان همچنان کار می‌کند:

```bash
cargo build --release
./target/release/linuprint --help
```

## شروع سریع

```bash
# اجرای ویزارد گرافیکی
linuprint gui

# بررسی وضعیت سیستم و دریافت راه‌حل
linuprint diagnose

# فهرست چاپگرها و اسکنرها
linuprint printers list
linuprint scanners list
```

## رابط گرافیکی

ویزارد پنج مرحله دارد:

1. **بررسی سیستم**: وضعیت CUPS، Avahi، SANE، دسترسی مدیریتی و گروه‌ها.
2. **چاپگرها**: کشف، افزودن، آزمایش و حذف صف چاپ.
3. **چاپ و مدیریت صف**: انتخاب صف، چاپ فایل، تعیین گزینه‌ها، مشاهده و لغو کارها.
4. **اسکنر**: انتخاب دستگاه، قالب، حالت رنگ و رزولوشن، اسکن و ذخیره.
5. **پایان**: نکات تکمیلی و میان‌برهای CLI.

در نوار بالای پنجره، دکمه **GitHub** مخزن پروژه را در مرورگر پیش‌فرض باز می‌کند. اگر مرورگر پیش‌فرض در دسترس نباشد، برنامه نشانی پروژه را نمایش می‌دهد.

## مرجع CLI

| فرمان | توضیح |
| --- | --- |
| `linuprint diagnose [--fix]` | گزارش وضعیت و در صورت نیاز، پیشنهاد یا اجرای اصلاحات |
| `linuprint services` | فعال‌سازی `cups.socket` و `avahi-daemon` |
| `linuprint printers list` | نمایش چاپگرهای کشف‌شده و صف‌های نصب‌شده |
| `linuprint printers add` | افزودن صف با URI و نام مشخص |
| `linuprint printers test <queue>` | ارسال صفحه آزمون PDF |
| `linuprint printers print <queue> <file>` | چاپ فایل با گزینه‌های اختیاری |
| `linuprint printers jobs [--queue <queue>]` | نمایش کارهای در انتظار |
| `linuprint printers cancel <job>` | لغو یک کار |
| `linuprint printers cancel-all <queue>` | لغو همه کارهای یک صف |
| `linuprint printers default <queue>` | تنظیم چاپگر پیش‌فرض |
| `linuprint printers pause <queue>` | توقف موقت صف |
| `linuprint printers resume <queue>` | فعال‌سازی دوباره صف |
| `linuprint printers remove <queue>` | حذف صف چاپگر |
| `linuprint scanners list` | نمایش اسکنرهای شناسایی‌شده |
| `linuprint scanners scan` | اسکن و ذخیره در PNG، JPEG یا PDF |
| `linuprint install <package...>` | نصب بسته‌های رسمی با pacman |

### نمونه‌ها

```bash
# افزودن چاپگر شبکه driverless
linuprint printers add \
  --uri ipp://192.168.1.20:631/ipp/print \
  --name hp-officejet

# افزودن چاپگر USB قدیمی؛ انتخاب خودکار درایور و PPD
linuprint printers add \
  --uri "usb://HP/LaserJet%201320%20series?serial=00CNGW53YHLB" \
  --name hp-laserjet-1320

# چاپ فایل با گزینه‌های رایج
linuprint printers print hp-laserjet-1320 ~/Documents/report.pdf \
  --copies 2 \
  --pages 1-3,5 \
  --duplex \
  --media A4

# مدیریت صف
linuprint printers jobs --queue hp-laserjet-1320
linuprint printers cancel hp-laserjet-1320-42
linuprint printers pause hp-laserjet-1320
linuprint printers resume hp-laserjet-1320

# اسکن در قالب PNG؛ بدون --output در ~/Documents ذخیره می‌شود
linuprint scanners scan \
  --device "escl:https://192.168.1.20:443" \
  --format png \
  --mode color \
  --resolution 300

# اسکن در قالب PDF با مسیر مشخص
linuprint scanners scan \
  --device "hpaio:/usb/OfficeJet?serial=1" \
  --output ~/Documents/scan.pdf \
  --format pdf \
  --mode gray \
  --resolution 300
```

برای دیدن همه گزینه‌ها:

```bash
linuprint --help
linuprint printers --help
linuprint scanners scan --help
```

## نحوه کار و امنیت

- فرمان‌ها بدون shell و به‌صورت آرگومان‌های جدا اجرا می‌شوند؛ ورودی کاربر هرگز به رشته shell تبدیل نمی‌شود.
- عملیات مدیریتی در GUI با `pkexec` و در CLI با `sudo` انجام می‌شود. برنامه رمز عبور را ذخیره یا مدیریت نمی‌کند.
- نام صف، شناسه کار، محدوده صفحه، اندازه کاغذ و مسیر فایل قبل از استفاده اعتبارسنجی می‌شوند.
- برای چاپگرهای IPP از `-m everywhere` استفاده می‌شود. برای USB و پروتکل‌های قدیمی، بسته رسمی درایور نصب و دقیق‌ترین PPD از `lpinfo -m` انتخاب می‌شود.
- بسته‌های AUR خودکار نصب نمی‌شوند؛ فقط فرمان پیشنهادی نمایش داده می‌شود.
- صفحه آزمون به‌صورت PDF در پوشه موقت ساخته و پس از ارسال حذف می‌شود.
- فایل‌های انتخابی چاپ توسط برنامه خوانده نمی‌شوند و فقط مسیر آن‌ها به `lp` داده می‌شود.
- فایل‌های اسکن‌شده فقط در مسیر انتخاب‌شده توسط کاربر ذخیره می‌شوند.
- تنها خواندن `/etc/nsswitch.conf` برای بررسی mDNS انجام می‌شود. تغییرات سیستم فقط از طریق ابزارهای رسمی CUPS/SANE/systemd و با تأیید polkit/sudo اعمال می‌شوند.

## عیب‌یابی

| مشکل | راه‌حل |
| --- | --- |
| چاپگر شبکه پیدا نمی‌شود | `systemctl enable --now avahi-daemon.service` را اجرا کنید، باز بودن UDP 5353 و نصب `nss-mdns` را بررسی کنید |
| نام‌های `.local` حل نمی‌شوند | `mdns_minimal` را به خط `hosts` در `/etc/nsswitch.conf` اضافه کنید |
| افزودن چاپگر شکست می‌خورد | `systemctl status cups.socket` و `journalctl -u cups` را بررسی کنید |
| چاپگر USB driverless کار نمی‌کند | برای چاپگرهای IPP روی USB بسته `ipp-usb` را نصب کنید؛ در غیر این صورت مدل را از `lpinfo -m` دستی وارد کنید |
| اسکنر پیدا نمی‌شود | `sane` و `sane-airscan` را نصب و `scanimage -L` را اجرا کنید |
| اسکنر دسترسی ندارد | عضو گروه‌های `scanner` و `lp` شوید و دوباره وارد شوید |
| خطای `No authentication agent found` | از یک نشست دسکتاپ با عامل polkit استفاده کنید یا عملیات را با CLI و `sudo` انجام دهید |
| صفحه چاپ آزمایشی سفید بیرون می‌آید | وضعیت کار را با `lpstat -W all -o` و `journalctl -u cups` بررسی کنید؛ اتصال کابل، وضعیت کارتریج/کاغذ و نصب `ghostscript` را کنترل کنید |
| لغو کار کاربر دیگر مجاز نیست | CLI را با `sudo` اجرا کنید یا از دکمه «لغو همه» در GUI استفاده کنید |
| اسکن با خطای mode متوقف می‌شود | حالت رنگ را روی «خودکار» بگذارید یا گزینه‌های معتبر دستگاه را با `scanimage -A -d <device>` ببینید |
| فایل اسکن در Documents ساخته نمی‌شود | وجود پوشه `~/Documents` و مجوز نوشتن را بررسی کنید؛ یا مسیر `--output` بدهید |
| باز شدن مرورگر یا CUPS ناموفق است | بسته `xdg-utils` را نصب کنید |
| درایور فقط در AUR موجود است | با `paru -S <package>` یا `yay -S <package>` نصب کنید |

گزارش کامل وضعیت:

```bash
linuprint diagnose
```

## توسعه

```bash
cargo fmt --all -- --check
cargo test
cargo clippy --all-targets -- -D warnings
```

برای بررسی رابط گرافیکی روی آرچ:

```bash
cargo test --features gui
cargo clippy --all-targets --features gui -- -D warnings
cargo build --release --features gui
```

اجرای آزمایشی بدون نمایشگر:

```bash
xvfb-run -a ./target/debug/linuprint gui
```

ساختار پروژه:

```text
src/
  backend/
    shell.rs        اجرای امن فرمان‌ها و مدیریت دسترسی (pkexec/sudo)
    printers.rs     کشف، افزودن، حذف، چاپ فایل و مدیریت کارهای صف
    avahi.rs        تجزیه خروجی avahi-browse و ساخت URI
    scanners.rs     کشف و اسکن با SANE، قالب، حالت رنگ و ذخیره خروجی
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

## بسته‌بندی و انتشار

`PKGBUILD` مستقیماً از تگ همان مخزن می‌سازد و `.SRCINFO` در مخزن نگه‌داری می‌شود. برای ساخت محلی:

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

با push یک تگ نسخه، گردش‌کار `Release` اجرا می‌شود:

```bash
git tag v0.2.0
git push origin v0.2.0
```

این گردش‌کار داخل کانتینر Arch بسته را با `makepkg` می‌سازد و فایل‌های `linuprint-0.2.0-1-x86_64.pkg.tar.zst` و `SHA256SUMS` را به release همان تگ اضافه می‌کند.

برای ساخت تگ از خود GitHub هم می‌توانید از مسیر **Actions → Release → Run workflow** و وارد کردن نسخه (مثلاً `v0.2.0`) استفاده کنید.

## مشارکت

- برای گزارش باگ یا پیشنهاد قابلیت، یک [issue](https://github.com/TheGreatAzizi/LinuPrint/issues) باز کنید و خروجی `linuprint diagnose` را (بدون اطلاعات حساس) ضمیمه کنید.
- برای تغییرات رفتاری یا معماری، بهتر است ابتدا issue باز شود تا رویکرد هماهنگ شود.
- قبل از ارسال Pull Request، `cargo fmt --all -- --check`، `cargo test` و `cargo clippy --all-targets --features gui -- -D warnings` را روی Arch اجرا کنید.
- فرمان‌ها باید بدون shell و با آرگومان‌های جدا اجرا شوند؛ آزمون‌های parser و command builder را برای تغییرات جدید اضافه کنید.
- توکن، رمز، نام کاربری یا خروجی حساس سیستم را در issue و commit قرار ندهید.

## مجوز

[MIT](LICENSE) © TheGreatAzizi

## English summary

LinuPrint is a GTK4 wizard for Arch Linux that simplifies printer and scanner setup using the standard **CUPS**, **Avahi**, **SANE**, and **pacman** tools. It discovers USB and network devices, configures CUPS queues with the appropriate driver or driverless mode, prints a generated PDF test page, manages print jobs, and scans to PNG, JPEG, or PDF.

Install the package from the [latest release](https://github.com/TheGreatAzizi/LinuPrint/releases), or build from source with `cargo build --release --features gui`. The graphical interface requires GTK 4.10 or newer. The CLI is available without GTK.

Contributions, bug reports, and feature requests are welcome.

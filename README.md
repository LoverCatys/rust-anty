# RUST ANTY

Нативный менеджер изолированных профилей Google Chrome на Rust с прямым подключением по Chrome DevTools Protocol (CDP), встроенным асинхронным прокси-мостом и контролем согласованности отпечатка.

## Возможности

- **Нативный Chrome без Selenium / ChromeDriver** — прямое управление через CDP WebSocket без включения `Runtime.enable`.
- **Асинхронный прокси-мост** — поддержка HTTP, HTTPS, SOCKS5 (с удалённым DNS) и SOCKS4/4a, автопроверка IP, геолокации и часового пояса (`ipwho.is` / `ipinfo.io` / `ip-api.com`).
- **Согласованность железа с хостом** — определение реального числа потоков CPU и вендора/архитектуры GPU (`src/host.rs`), подбор совместимых пар CPU+GPU и предупреждения при несоответствиях.
- **Стелс-консистентность** — синхронный патчинг основных окон, `<iframe>` (`contentWindow`, `window[0]`, `insertAdjacentHTML`, `srcdoc`, `document.write`, `window.open`) и Web Workers (`OffscreenCanvas`, таймзона, локаль), детерминированный шум Canvas 2D / AudioBuffer, защита от утечек WebRTC.
- **Импорт и экспорт Cookies** — поддержка форматов JSON и Netscape HTTP Cookie File как для запущенных (`[ONLINE]`), так и для оффлайн-профилей через нативный CDP.
- **Шифрование паролей прокси** — на Windows пароли в `profiliki.json` шифруются через Windows DPAPI (`CryptProtectData`).

## Сборка и запуск

Требуется установленный [Rust](https://rustup.rs/) и Google Chrome.

```bash
cargo build --release
```

Запуск на Windows:

```bat
start_rust_anty.bat
```

Или напрямую:

```bash
cargo run --release
```

## Тесты

Включают юнит-тесты и интеграционные проверки на живом Chrome (CDP stealth probe + импорт/экспорт cookies):

```bash
cargo test --bin rust-anty -- --nocapture
```

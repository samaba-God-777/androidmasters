# AndroidMasters

Plataforma de escritorio para técnicos: identificación de dispositivos Android por USB, diagnóstico de cuentas y de **Google FRP**, procedimientos oficiales de fabricantes, restablecimiento autorizado, preparación para nuevos propietarios y órdenes de servicio con reportes.

> FRP es una protección antirrobo. AndroidMasters **no** incluye ni incluirá bypass, exploits ni mecanismos para evadir verificaciones de propiedad. Si no hay prueba de propiedad o autorización, el proceso se detiene.

Stack: Tauri 2 (Rust) · React 19 + TypeScript + Vite + Tailwind 4 · Lucide · SQLite. Ver [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) y [docs/PHASES.md](docs/PHASES.md).

## Requisitos

- Node.js 22+, Rust 1.80+ (`rustup`)
- macOS: Xcode Command Line Tools · Windows: Microsoft C++ Build Tools y WebView2 (incluido en Windows 11)
- Android SDK Platform-Tools (`adb`) para trabajar con teléfonos

## Desarrollo

```bash
npm install
```

```bash
npm run tauri dev
```

## Pruebas

```bash
npm test
```

```bash
npm run test:rust
```

## Instaladores

```bash
tools/build_installers.sh
```

Genera en `~/Downloads`:

- `AndroidMasters-1.0.0-macOS-universal.dmg`: macOS 11+ en Intel y Apple Silicon. Arrastre la app a Aplicaciones.
- `AndroidMasters-1.0.0-Windows-x64-setup.exe`: Windows 10 y 11 (64 bits). Se instala para el usuario actual, sin permisos de administrador. WebView2 se descarga si falta.

Requisitos para compilarlos desde un Mac: Xcode 26+ (icono), los targets de Rust `x86_64-apple-darwin`, `aarch64-apple-darwin` y `x86_64-pc-windows-msvc`, `cargo-xwin`, y `llvm` y `nsis` de Homebrew.

Los instaladores **no están firmados** con un certificado de desarrollador:

- macOS: la app lleva firma ad hoc. La primera vez, abra la app con clic derecho › Abrir, o en Ajustes › Privacidad y seguridad › «Abrir igualmente».
- Windows: SmartScreen avisa de «editor desconocido». Pulse «Más información» › «Ejecutar de todas formas».

Para distribuir a otros talleres, firme con un Developer ID de Apple (y notarice) y con un certificado de firma de código de Windows.

El icono está en `Icon/AppIcon.icon` (Icon Composer, Liquid Glass). `tools/make_icon.sh` lo compila y `tools/icon_review.sh` lo revisa. Proceso adaptado de la skill `app-icon` de [daveebbelaar/bama](https://github.com/daveebbelaar/bama) (MIT).

## Seguridad

Ver [docs/SECURITY.md](docs/SECURITY.md).

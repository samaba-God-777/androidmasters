# AndroidMasters — Arquitectura

Guía principal: la Skill `androidmasters-device-management` (SKILL.md). Toda funcionalidad respeta sus reglas: distinguir **Diagnóstico / Instrucción / Acción ejecutada / Verificación**, flujo obligatorio de 10 pasos, procedimientos oficiales versionados, consentimiento del propietario, y prohibición absoluta de evadir FRP o protecciones antirrobo.

## Capas

```
React + TypeScript (src/)                       ← solo UI y estado de pantalla
   │  invoke() tipado (src/services/api.ts) — errores normalizados { code, message }
   ▼
Comandos Tauri (src-tauri/src/commands)         ← única superficie expuesta; valida entradas
   ▼
Servicios (src-tauri/src/services)              ← lógica: ADB/USB, FRP, cuentas, reset, órdenes, reportes
   ▼
Seguridad (src-tauri/src/security)              ← política de comandos, cifrado, contraseñas
Base de datos (src-tauri/src/database)          ← SQLite + migraciones versionadas (src-tauri/migrations)
```

- La UI **no** puede ejecutar procesos ni leer archivos: la *capability* de Tauri solo concede `core:default` y los comandos propios.
- CSP estricta (`script-src 'self'`, sin `eval`, sin orígenes externos) y `freezePrototype`.
- Todo `adb`/`fastboot` pasa por `security::policy` (lista blanca). Lo relacionado con FRP, cuentas, setup wizard, IMEI/baseband, Knox/Find My Device o bloqueos de operador está prohibido de forma permanente.

## Mapa de carpetas

| Pedido | Ubicación | Nota |
|---|---|---|
| `src/components`, `pages`, `modules`, `hooks`, `services`, `types`, `utils`, `styles` | `src/…` | Frontend |
| `src-tauri/commands`, `services`, `database` | `src-tauri/src/commands`, `src/services`, `src/database` | Rust exige que los módulos vivan bajo `src/` |
| `src-tauri/migrations` | `src-tauri/migrations` | SQL embebido en el binario con `include_str!` |
| `tests` | `tests/` (frontend, Vitest) y `#[cfg(test)]` en cada módulo Rust | |

## Decisiones técnicas

| Tema | Decisión | Motivo |
|---|---|---|
| Escritorio | Tauri 2 (estable) | 3.0 está en alfa |
| SQLite | `rusqlite` con SQLite embebido (`bundled`) | Sin dependencias del sistema en macOS/Windows |
| Migraciones | Archivos SQL numerados, tabla `schema_migrations`, una transacción por migración | Simples, auditables; nunca se edita una migración aplicada |
| Copias de seguridad | API *backup* de SQLite + `integrity_check` de la copia; nunca sobrescribe | Copia consistente con la app abierta |
| Series de dispositivos | ChaCha20-Poly1305 + huella HMAC-SHA256; se muestran enmascaradas | Búsqueda sin texto claro; la clave está en `secret.key` (0600) |
| Contraseñas de técnicos | Argon2id, mínimo 10 caracteres | Las contraseñas de los propietarios nunca pasan por la app |
| Auditoría | Tabla `audit_log` solo-anexar (triggers bloquean UPDATE/DELETE) | Trazabilidad exigida por la Skill |
| Instrucciones | `CHECK` en `order_entries`: una instrucción no puede marcarse como ejecutada | "Nunca presentar una guía como operación completada" |
| Datos de cuentas | Solo tipos y cantidades (p. ej. "Google: 1"); nunca nombres ni correos | Privacidad |
| Relación con Android Master Repair (Python) | Proyecto independiente; se reutilizan y traducen la política de comandos, los parsers y los procedimientos ya probados allí | Evita reinventar lógica verificada |

## Datos de la aplicación

- macOS: `~/Library/Application Support/com.androidmasters.desktop/`
- Windows: `%APPDATA%\com.androidmasters.desktop\`

Contiene `androidmasters.sqlite3`, `secret.key` y `backups/`. **Sin `secret.key` las series cifradas no se pueden leer**: inclúyala en las copias externas del equipo.

## Usuarios, roles y sesión (fase 2)

- Primer arranque: se crea la cuenta de **administrador** (no existen credenciales predeterminadas). Después, solo un administrador crea cuentas.
- **Técnico**: trabajo de servicio. **Administrador**: además usuarios, copias de seguridad, revisión de procedimientos y auditoría. Siempre queda un administrador activo y nadie se desactiva a sí mismo.
- La sesión vive en el backend (`AppState`); cada comando vuelve a leer el usuario, así que un cambio de rol o una desactivación se aplican en la siguiente acción.
- 5 intentos fallidos bloquean la cuenta 15 minutos; el mensaje de error no revela si el usuario existe. Todo queda en `audit_log`.

## Interfaz (fase 2)

- Tema oscuro con un solo color de acento (verde azulado) reservado para lo interactivo y el estado activo; números tabulares; paneles planos sin anidar.
- Movimiento solo con propósito: pulsación de botones (`scale(0.97)`, 160 ms), diálogos y avisos (≈200 ms, `ease-out` fuerte, salida más rápida que la entrada). La navegación no se anima. Se respeta `prefers-reduced-motion`.
- Diálogos con `<dialog>` nativo (foco atrapado, Esc). Las acciones destructivas usan `ConfirmDialog` y, cuando corresponde, exigen escribir una frase.
- Los módulos de fases futuras muestran qué harán y en qué fase llegan; no ofrecen botones ni datos simulados.

## Carpeta de datos de desarrollo

Solo en compilaciones de desarrollo, `ANDROIDMASTERS_DATA_DIR=/ruta` usa una carpeta de datos desechable (pruebas, capturas). Las compilaciones de producción la ignoran.

## Detección USB y ADB (fase 3)

- `services/adb/runner.rs`: ejecuta procesos como lista de argumentos (sin shell), con tiempo límite; en Windows sin ventana de consola.
- `services/adb/locator.rs`: busca `adb`/`fastboot` en la carpeta configurada, `ANDROID_HOME`/`ANDROID_SDK_ROOT`, el SDK estándar, Homebrew, winget y `PATH`.
- `services/adb/client.rs`: **toda** llamada pasa por `security::policy` antes de ejecutarse; este cliente solo admite comandos de lectura o cambio de estado (nada destructivo).
- `services/devices.rs`: un hilo revisa el USB cada 2 s (`device_poll_interval_ms`, 1–30 s). Ante un equipo nuevo o un cambio de modo lo identifica (getprop, batería, almacenamiento; fastboot `getvar` en bootloader), emite `devices://snapshot` y `devices://event`, y guarda solo los equipos realmente identificados (serie cifrada) con una operación «Identificación del dispositivo».
- La interfaz recibe un `id` anónimo (prefijo de la huella HMAC) y la serie enmascarada; la serie completa solo la ve un administrador a petición, con registro en auditoría.
- Los campos que el equipo no informa se muestran como «No detectado» y se listan en `missing`; nunca se rellenan con valores supuestos.
- `services/usb.rs`: diagnóstico de conexión construido solo con lo que responden adb y el sistema (herramientas, servidor, controladores/udev, autorización, tipo de conexión, latencia).

## Diagnóstico FRP (fase 4)

- `services/accounts.rs`: lee `dumpsys account` y conserva **solo tipos y cantidades** («Google: 1»); los nombres/correos se descartan dentro del analizador y nunca salen de él. La consola libre no puede ejecutar esta consulta (`INTERNAL_ONLY` en la política).
- `services/frp.rs`:
  - `indicators`: datos de solo lectura del equipo conectado (versión con FRP, partición de protección declarada, cuentas Google y de fabricante con protección). Sin ADB (por ejemplo tras un restablecimiento) explica el límite y pide registrar lo que muestra la pantalla.
  - `assess`: lógica pura. Primero la autorización (si no hay, `stop` y documentación a presentar); después el camino oficial: inicio de sesión del propietario, periodo de seguridad tras cambio de contraseña, recuperación de Google o soporte del fabricante. Las causas posibles se derivan de las respuestas.
  - `record`: guarda la evaluación (`frp_assessments`), una operación `frp` (`rejected` cuando se detiene) y un evento de auditoría. Las evaluaciones incompletas no se guardan.
- La interfaz separa visualmente **Diagnóstico** (causas) e **Instrucción para el propietario** (pasos), y solo enlaza recursos oficiales de Google para que el propietario los abra.
- No existe ni existirá código para saltar, desactivar o eliminar FRP.

## Cuentas y fabricantes (fase 5)

- `src-tauri/procedures/*.json`: un archivo por fabricante (Google general, Samsung, Xiaomi/Mi, Motorola, Huawei, OPPO, realme, vivo, Tecno, Infinix, Google Pixel), embebidos en el binario. Cada procedimiento tiene `version`, pasos, cómo verificarlo y enlaces oficiales `https`. Son **instrucciones para el propietario**, nunca comandos. Ver `src-tauri/procedures/README.md`.
- Revisión del taller (`procedure_reviews`): un administrador marca cada procedimiento como revisado tras comprobarlo en un equipo real. La revisión guarda la versión: si el procedimiento cambia, vuelve a «Pendiente de revisión».
- La marca del equipo conectado se detecta por `ro.product.brand`/`ro.product.manufacturer` (Redmi y POCO → Xiaomi; Google → Pixel) y se preselecciona.
- Comprobación de cuentas (`account_checks`, migración 2): guarda solo tipos y cantidades; comparada con la anterior, confirma el retiro («Retiro de cuentas verificado»).
- Lista de transferencia (`checklists`): los puntos «Cuentas Google retiradas» y «Cuentas del fabricante cerradas» solo se marcan con evidencia: la última comprobación del equipo sin cuentas o, si no está conectado, una descripción escrita de lo verificado en pantalla. Una lista completa queda cerrada y se audita. La fase 6 la exigirá antes de restablecer.

## Restablecimiento autorizado (fase 6)

- **AndroidMasters nunca borra el teléfono.** El borrado lo confirma el propietario o el técnico en el propio equipo por el camino oficial (Ajustes o recovery).
- `services/reset.rs` (tabla `resets`, migración 3):
  1. `preflight`: equipo identificado (o marca y modelo escritos, con aviso), autorización registrada, lista de transferencia completa y última comprobación sin cuentas. Cualquier fallo bloquea `prepare`.
  2. `prepare`: crea el registro `prepared`. Solo puede haber uno abierto por lista de preparación.
  3. `Challenges`: token de un solo uso en memoria (5 min) con la frase `BORRAR-{id}`. `confirm` exige el token, la frase exacta y el nombre de quien autoriza; un `CHECK` de la base impide un estado posterior sin `confirmed_at`.
  4. `record_action`: «Abrir Ajustes» o «Reiniciar en recovery» (ADB, mismo equipo, ≤30 min desde la confirmación) o «El borrado se inició en el teléfono». Pasa a `pending_verification`.
  5. `verify`: el técnico registra lo que muestra la pantalla. `verified` solo si aparece el asistente de configuración y no pide cuenta anterior; si no, `failed` con orientación oficial (FRP: el propietario inicia sesión con su cuenta).
- Cada paso queda en `audit_log` y en `operations` (categoría `reset`). La interfaz separa **Instrucción** (pasos en el teléfono) de **Verificación**.

## Órdenes de servicio y reportes (fase 7)

- `services/orders.rs` (migración 4: `device_label` en la orden y `source` único por orden en las entradas):
  - Orden `OS-AAAA-NNNN` con cliente, equipo (identificado por USB o marca y modelo escritos), tipo de servicio y problema.
  - **Consentimiento**: concedido (nombre y forma de comprobar la propiedad), no concedido (añade una incidencia: servicio detenido) o revocado.
  - **Entradas**: el técnico escribe diagnósticos, instrucciones, verificaciones, incidencias y notas. Las **acciones ejecutadas** y los consentimientos solo llegan de los registros de la aplicación. Cada tipo admite solo ciertos estados (una instrucción nunca es «correcta»). Una verificación puede cerrar una entrada pendiente.
  - **Estados**: Abierta → En curso (exige consentimiento) → Pendiente de verificación → Completada (sin pendientes y con trabajo real, o con incidencia si no hubo consentimiento) → Entregada. Solo un administrador cancela, con motivo. Entregadas y canceladas no admiten cambios.
  - **Vincular trabajo**: propone diagnósticos FRP, comprobaciones de cuentas, listas completas y restablecimientos del mismo equipo (por id o por marca y modelo sin distinguir mayúsculas). Conserva la hora original, no duplica y encadena «borrado iniciado» → «verificado». Un borrado iniciado a mano en el teléfono se registra como nota, no como acción ejecutada.
- `services/pdf.rs`: generador PDF propio (A4, Helvetica con WinAnsi para el español, ajuste de líneas con métricas reales, saltos de página, pie con «Página X de N»). Sin dependencias.
- `services/reports.rs`: reporte por orden (cliente y equipo con serie enmascarada, consentimiento, problema, aviso de pendientes, registro con el tipo de cada entrada, firmas, nota de privacidad), resumen y PDF del periodo, CSV (`;`, BOM UTF-8, fórmulas neutralizadas, sin datos de contacto).
- Los archivos se guardan en `Descargas/AndroidMasters` (en desarrollo con `ANDROIDMASTERS_DATA_DIR`, en `reports/` de esa carpeta) y nunca se sobrescriben. «Mostrar en la carpeta» solo acepta archivos de esa carpeta.
- El nombre del taller para el encabezado lo cambia un administrador (`workshop_name`).

## Seguridad, icono e instaladores (fase 8)

- Sesión: `AppState` guarda el usuario y su última actividad. Tras `SESSION_IDLE` (30 min) sin comandos, `require_user` devuelve `session_expired`, se audita y la interfaz vuelve a la pantalla de inicio con el motivo (`SESSION_EXPIRED_EVENT`).
- La carpeta de datos se crea con permisos 0700. Ver [SECURITY.md](SECURITY.md).
- Icono: `Icon/AppIcon.icon` (Icon Composer). `tools/make_icon.sh` genera `src-tauri/icons/macos/Assets.car` (Liquid Glass, incluido en el bundle con `CFBundleIconName`) y `icon.icns`. Los PNG e `icon.ico` salen del render de Icon Composer con `tauri icon`.
- `tools/build_installers.sh [mac|windows|all]`: DMG universal y setup NSIS x64 (compilado desde el Mac con `cargo-xwin`), copiados a `~/Downloads`.


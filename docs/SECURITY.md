# Seguridad y privacidad

AndroidMasters ayuda a técnicos autorizados. **No** evade FRP ni otras protecciones antirrobo, no elimina cuentas sin el propietario y no extrae credenciales. Todo lo que hace el teléfono pasa por una lista blanca de comandos.

## Comandos al teléfono

- `security/policy.rs`: lista blanca de `adb`, `adb shell` y `fastboot`. Todo lo demás se rechaza. Los comandos destructivos nunca salen del cliente ADB. `dumpsys account` solo se usa internamente.
- Los procesos se ejecutan sin shell, como lista de argumentos y con tiempo límite.
- El restablecimiento **no** lo ejecuta la aplicación. La aplicación exige comprobaciones previas, una confirmación de un solo uso (5 min, frase `BORRAR-n`, nombre de quien autoriza) y una verificación posterior. Las ayudas «Abrir Ajustes» y «Reiniciar en Recovery» solo funcionan en el mismo equipo confirmado, dentro de los 30 minutos siguientes.

## Datos

- La carpeta de datos solo es accesible para el usuario del sistema (0700). `secret.key` tiene permisos 0600.
- Números de serie: cifrados con ChaCha20-Poly1305, más una huella HMAC para buscarlos. La interfaz solo ve un id anónimo y la serie enmascarada. La serie completa la ve un administrador, y queda auditado.
- Cuentas: solo tipos y cantidades. Los nombres y correos se descartan dentro del analizador.
- Nunca se guardan contraseñas del propietario, PIN, códigos MFA ni tokens. Los formularios lo recuerdan.
- Reportes: serie enmascarada y nota de privacidad. El CSV neutraliza fórmulas y no incluye datos de contacto.

## Usuarios

- Contraseñas con Argon2id. Bloqueo tras intentos fallidos con mensaje genérico. Roles administrador y técnico.
- **La sesión se cierra tras 30 minutos sin actividad**, y queda auditado. La interfaz vuelve a la pantalla de inicio y explica el motivo.
- Solo existen cuatro comandos sin sesión: estado, primer administrador (solo si no hay usuarios), iniciar sesión y cerrar sesión. Todos los demás exigen sesión, y los de administración exigen el rol.

## Registro

- `audit_log` solo admite añadir: la base de datos rechaza modificar o borrar.
- Una instrucción no puede registrarse como ejecutada (restricción `CHECK`). Las acciones ejecutadas y los consentimientos de las órdenes solo los registra la aplicación.

## Interfaz

- CSP estricta (`default-src 'self'`, sin scripts externos ni `eval`). Permisos de Tauri mínimos (`core:default`). Sin acceso a archivos ni shell desde la web.
- «Mostrar en la carpeta» solo abre archivos de la carpeta de reportes.
- `ANDROIDMASTERS_DATA_DIR` (carpeta de datos desechable) solo funciona en compilaciones de desarrollo.

## Pendiente antes de distribuir a terceros

- Firmar y notarizar con certificados reales (macOS Developer ID, Authenticode).
- Auditar las dependencias de Rust con `cargo audit` (no se pudo instalar en este equipo). `npm audit`: 0 vulnerabilidades.
- Revisar los procedimientos de los fabricantes en teléfonos reales y probar con un teléfono conectado (ver PHASES.md).

# Plan de implementación: investigación web iterativa de Neeko

Fecha de revisión: 2026-09-14. Workspace: `D:\NEEKO API\neeko-assistant`.

## 1. Encargo y estado de entrega

El usuario quiere que Neeko investigue con un comportamiento comparable al que observa en ChatGPT: decidir qué buscar, descubrir fuentes oficiales sin una tabla de marcas, abrir páginas, seguir enlaces, reformular consultas con lo aprendido y responder con evidencia. Pidió un plan suficientemente detallado para que otra IA continúe al agotarse el contexto.

**Este archivo es el plan y el documento de traspaso. La nueva implementación todavía está pendiente.** La revisión se hizo leyendo el código local; no se realizaron evaluaciones en vivo de investigación en este turno. No presentar los pasos de este documento como cambios ya ejecutados.

No se intenta reproducir una implementación interna de OpenAI. Se especifica una arquitectura propia con herramientas, control de presupuesto y evidencia verificable. La calidad depende del modelo local, los proveedores de búsqueda y el contenido accesible; no prometer equivalencia de resultados.

La próxima IA debe implementar el plan, verificarlo y actualizar la sección de seguimiento. No necesita volver a preguntar si el usuario quiere esta funcionalidad. Si el usuario solicita solo revisar el plan, respetar esa nueva instrucción.

## 2. Restricciones del proyecto y continuidad

- Mantener el modelo y servidor local configurados. No introducir OpenAI API, servicios pagos, credenciales obligatorias, embeddings ni otro modelo por defecto.
- Conservar Google mediante Edge aislado como proveedor inicial y las APIs existentes. Diseñar interfaces para sustituir proveedores sin rehacer el agente.
- Mantener `source_research_enabled`; con el ajuste desactivado no buscar ni emitir estados de investigación.
- Mantener los comandos locales, la aprobación de acciones y el flujo de addons. La investigación es de lectura y no debe convertirse en un camino para ejecutar acciones.
- Respetar idioma y contexto de seguimiento. No enviar historial completo, secretos o información privada innecesaria a buscadores.
- El usuario prefiere mensajes casuales, útiles y breves. Mostrar actividad real, sin simular progreso ni exponer razonamientos internos.
- Hay muchos cambios previos sin commit. No usar reset/checkout global ni formatear archivos ajenos. Revisar `git status --short` antes de editar y preservar todos los cambios preexistentes.
- No modificar TikTok ni sus motores como parte de esta tarea. Tampoco alterar Shazam: el aviso aprobado es «Voy a escuchar lo que suena en la PC durante 10 segundos. Esperame un momento…».
- No compilar instaladores, publicar, subir commits ni cambiar versión para entregar esta mejora salvo que se solicite. Una comprobación de Rust no actualiza la app instalada; informar si requiere reconstrucción/reinicio.
- No hay autorización para enviar mensajes a terceros. Consultar fuentes públicas es parte de la investigación; publicar o interactuar con cuentas no lo es.

## 3. Estado actual verificado

Las líneas son aproximadas: buscar los símbolos, porque el árbol tiene cambios activos.

| Archivo / símbolo | Función actual | Limitación relevante |
|---|---|---|
| `src-tauri/src/assistant.rs`, `response_schema`, `parse_reply`, prompt principal | El modelo puede proponer `research` con intención y 1–3 consultas | El contrato no tiene operaciones de lectura o navegación |
| Mismo archivo, `assistant_chat`, bucle `research_rounds < 2` | Busca, sintetiza y acepta una segunda ronda | Límite fijo; no es un bucle de herramientas |
| `research_model` | Solicitud JSON al modelo configurado, timeout 45 s | Reutilizar integración; no asumir soporte de function calling nativo |
| `synthesize_research_answer` | Selecciona hasta 3 de 8 candidatos, lee y responde | Sustituye el vector por seleccionadas, renumera IDs y recorta evidencia a 2400 caracteres; escritor de 384 tokens |
| `src-tauri/src/research.rs`, `search_sources` | Hasta 3 consultas, máximo 8 candidatos por lote | La primera consulta puede consumir gran parte del cupo; un fallo puede abortar el lote |
| `scoped_terms` | Filtros por sitios y palabras | `official/docs` agrega `official documentation`; `news` agrega Reuters/AP/BBC. No hay descubrimiento oficial explícito |
| `github_sources` | Busca repositorios por API | Devuelve descripción, no README ni releases; `updated_at` aparece como `published_at` |
| `reddit_sources` | Busca posts por API | Devuelve título/selftext, no comentarios |
| `read_sources` | Lee hasta tres fuentes | Omite todas las `api_extract` y videos; por eso GitHub/Reddit de API no se amplían |
| `research/reader.rs` | HTTP y extracción HTML/texto, fragmentos relevantes de hasta 6000 caracteres | No retorna enlaces ni secciones navegables; no procesa PDF ni páginas JS |
| `research/google.rs` | Edge headless, perfil temporal, semáforo global de un navegador y cierre mediante Job Object | Dependencia de Edge, buscador puede bloquearse; conservar limpieza/cancelación |
| `research/cache.rs` | Caché en memoria por consultas/intención/idioma, TTL variable | No conserva documentos completos ni verifica dominios; intención libre puede caer en TTL genérico de 12 h |
| `src/chat-client.mjs`, `startProgress` | Cambia thinking/searching/reading/preparing cada 900 ms | Estados ficticios independientes del backend |
| `src/main.js` y `src/chat.js` | Chat de mascota y ventana, invocaciones Tauri | Sesiones con prefijos `desktop:` y `chat-window:` |
| `web/index.html`, `getChatClient` | Usa el mismo ChatClient | Callback `message(message, info)` ignora `sources`; no tiene progreso de investigación real |
| `src-tauri/src/web_server.rs` | Rutas de chat/decide/cancel, prefijo `web:` en backend | Agregar transporte de progreso sin perder aislamiento y controles existentes |
| `src-tauri/src/config.rs` | `source_research_enabled`, por defecto false | Nuevos campos deben deserializar configuraciones antiguas |
| `docs/research-vane.md` | Documenta versión actual de dos rondas | Actualizar al terminar; no dejar documentación contradictoria |

El prompt ya pide fuentes primarias relevantes, diversidad y reconocer desacuerdos. La validación de citas comprueba IDs existentes, **no** que el texto de la fuente respalde cada afirmación. El extractor selecciona fragmentos; incluso su estado `full` no debe interpretarse como que el modelo recibió la página entera.

## 4. Resultado funcional esperado

```text
Mensaje + contexto relevante
  -> decidir si requiere web / resolver referente de la pregunta
  -> objetivo, entidades, datos por verificar y vigencia necesaria
  -> modelo elige una operación
       buscar / leer / seguir enlace / consultar recurso estructurado
  -> backend valida, aplica límites y ejecuta
  -> evidencia y enlaces se incorporan al estado persistente de la consulta
  -> modelo evalúa lo pendiente y elige otra operación o terminar
  -> redactar con referencias a evidencia
  -> validar referencias y revisar respaldo
  -> respuesta + fuentes, o respuesta parcial con limitación concreta
```

No obligar a cinco sitios, a tres fuentes ni a consumir todo el presupuesto. Una release oficial inequívoca puede bastar para la versión de un proyecto. Una comparación o una controversia puede necesitar varias fuentes independientes. Dos artículos que repiten el mismo comunicado no son dos confirmaciones independientes.

Ejemplos de aceptación:

1. Versión de un proyecto desconocido: búsqueda amplia -> identidad del proyecto -> repositorio oficial -> release estable -> versión, fecha, URL. Distinguir prerelease de estable y fecha de release de última modificación del repo.
2. Capacidad de una GPU: descubrir fabricante -> abrir especificación del modelo exacto -> responder solo lo que documenta. No extrapolar automáticamente desde otra GPU.
3. Fallo de software: identificar versión/plataforma -> docs/issues/comunidad -> separar reporte anecdótico, bug confirmado y arreglo publicado.
4. Pregunta de seguimiento «¿y en Linux?»: resolver a qué producto se refiere antes de construir la consulta y conservar el contexto en todos los pasos.
5. Fuente inaccesible: mostrar la limitación o buscar otra; nunca afirmar que se leyó su contenido.

## 5. Modelo de datos propuesto

Crear `src-tauri/src/research/types.rs`. Nombres sugeridos; puede adaptarse la sintaxis a convenciones existentes conservando estos contratos.

### Estado por investigación

`ResearchState` debe contener:

- `request_id`, sesión interna y generación; no confiar en IDs suministrados como prueba de autorización.
- Pregunta original y `resolved_question` autosuficiente, idioma y fecha actual obtenida del sistema.
- `intent` como enum validado: general_fact, current_version, documentation, hardware_capability, troubleshooting, user_experience, comparison, news, etc.; desconocido -> general_fact.
- `entities`: nombre, tipo, fabricante/proyecto si se conoce, variantes y relación aún no verificada. Son hipótesis hasta que haya evidencia.
- `questions`: datos concretos necesarios, estado pendiente/respondido/contradictorio y referencias que los respaldan.
- Registro acumulativo de candidatos, documentos, enlaces, fragmentos y errores. No borrar fuentes por no seleccionarlas en un paso.
- Historial compacto de operaciones y sus resultados; no guardar cadena de pensamiento.
- Presupuesto consumido, deadline monotónico, operaciones repetidas y motivo de finalización.

### Fuentes y evidencia

- IDs estables asignados por backend: `s1`, `s2`, etc., nunca por el modelo ni renumerados durante la investigación.
- Separar metadatos de resultado de búsqueda de contenido efectivamente obtenido.
- Documento: URL pedida, URL final, título, dominio, método/proveedor, estado HTTP, fecha de recuperación, fecha de publicación y modificación **separadas**, alcance de lectura y enlaces descubiertos.
- Estados tipados: search_snippet, api_metadata, content_read, partial, blocked, unsupported, failed. La descripción de un repo es api_metadata, no lectura del README.
- Fragmentos con IDs estables como `s2:p4` y texto exacto; añadir sección o localización si existe. El modelo referencia fragmentos, no crea citas textuales.
- Enlaces `link_id`, URL absoluta validada y texto del enlace. Resolver relativos contra URL final, deduplicar sin eliminar parámetros que cambien el recurso.
- Autoridad con relación a la entidad y afirmación: fabricante/proyecto oficial confirmado, candidato oficial, fuente secundaria, comunidad. Guardar por qué y qué evidencia lo indica; no inventar un porcentaje de confianza.
- Normalizar fechas a ISO 8601 cuando sean fechas verificables; preservar valor original si no se puede interpretar. Nunca usar recuperación o modificación como fecha de publicación sin etiquetarla.

Mantener `Reply.sources` compatible mediante una proyección del estado a las fuentes realmente citadas. Si cambia la numeración visual, crear un mapeo final único y aplicarlo tanto al mensaje como a la lista. Un ID válido es necesario pero no suficiente para acreditar una afirmación.

## 6. Herramientas internas y validación

Crear `research/tools.rs` y un enum serde de operaciones. Utilizar salida JSON restringida compatible con el servidor actual. No exigir tool calling nativo ni mezclar estas herramientas con `action-catalog.json`.

Operaciones mínimas:

| Operación | Argumentos | Resultado |
|---|---|---|
| `search` | query, estrategia, dominios opcionales, vigencia opcional | Candidatos reales, snippets y errores por proveedor |
| `read` | source_id y foco opcional | Documento, fragmentos y enlaces |
| `follow_link` | source_id, link_id, foco opcional | Nuevo documento con procedencia del enlace |
| `github_read` | source_id, recurso README/releases/issues, filtros limitados | Contenido/API con URLs citables y fechas correctas |
| `reddit_read` | source_id, máximo de comentarios y orden validado | Post/comentarios con permalinks, sin tratar votos como veracidad |
| `finish` | estado complete/partial/insufficient y datos pendientes | Solicita redacción; no elude la verificación |

Ejemplo de decisión interna, no contrato existente:

```json
{"operation":"follow_link","source_id":"s2","link_id":"l3","focus":"última release estable"}
```

Validar campos, tipos, longitudes, enums e IDs antes de ejecutar. `read` solo admite candidatos conocidos; `follow_link` solo enlaces del documento indicado. Una URL explícita del usuario puede incorporarse al registro con procedencia user_provided tras validación. Los endpoints GitHub/Reddit se construyen en backend desde una identidad validada, no desde comandos o URLs arbitrarios del modelo.

Devolver errores estructurados recuperables: no_results, blocked, timeout, unsupported_format, invalid_reference, rate_limited, budget_exhausted. Un proveedor caído no debe borrar resultados válidos de otra consulta. No reintentar de forma infinita.

## 7. Búsqueda y descubrimiento de fuentes oficiales

Separar **estrategia de evidencia** de **proveedor de búsqueda**. `official` debe orientar el objetivo; no limitarse a agregar las mismas dos palabras inglesas a cualquier pregunta. `domain` necesita un dominio validado o un filtro de búsqueda explícito.

Proceso:

1. Empezar con la entidad y el dato solicitado, con el idioma útil para esa entidad.
2. Leer candidatos que parezcan primarios y revisar nombre del producto, identidad y relaciones entre sitio y repositorio.
3. Tratar el dominio como candidato hasta contar con indicios suficientes, por ejemplo enlace del sitio del proyecto al repositorio. Nombre similar o primer resultado no bastan.
4. Si sirve al objetivo, buscar dentro del dominio descubierto o seguir documentación/releases enlazadas.
5. Registrar ambigüedad si hay forks, homónimos o dominios dudosos. No bloquear toda respuesta por no conseguir una prueba perfecta, pero no llamar oficial a una fuente sin fundamento.

Eliminar la preferencia obligatoria Reuters/AP/BBC de toda consulta news; permitir búsqueda pertinente al país, tema e idioma. Conservar las búsquedas explícitas del usuario en un sitio. Las estrategias no deben transformarse en una nueva lista rígida de marcas.

No agregar caché persistente de identidad oficial en la primera versión. Si luego se agrega, tendrá evidencia, caducidad y capacidad de invalidación, nunca una asociación permanente aprendida de un único snippet.

## 8. Lectura y navegación

Ampliar `research/reader.rs` para devolver texto por bloques y enlaces, además de metadatos. Leer una página y elegir fragmentos para el modelo son pasos diferentes: conservar el texto acotado disponible para cambiar el foco sin descargarla otra vez.

- No omitir automáticamente `api_extract`: decidir si ese extracto contiene el dato necesario o requiere ampliar contenido.
- GitHub: README con formato/encoding correcto; releases con tag, título, fecha, prerelease/draft y URL. Issues incluyen su tipo, cuerpo y estado; comentarios de forma acotada. API pública sin token inicialmente, con manejo de 403/429 y límites.
- Reddit: post y un conjunto acotado de comentarios legibles, cada uno con enlace, fecha y contenido. Si el acceso falla, usar otra fuente o snippets etiquetados. No prometer acceso a contenido borrado/privado.
- HTML: excluir navegación/script, conservar encabezados útiles, enlaces y tablas relevantes. Evitar duplicar bloques anidados.
- PDF/JS/video: estados explícitos de contenido no soportado si no hay lector. No fingir haber visto un video ni usar su título como benchmark. PDF y renderizado JS de páginas arbitrarias quedan como extensión futura; la primera entrega completa del plan debe indicar estas limitaciones.
- Textos grandes: dividir en fragmentos y seleccionar por foco. No truncar siempre el inicio, perdiendo la release o especificación relevante.

Antes de ampliar URLs dinámicas corregir dos limitaciones del lector actual: obtiene `.bytes()` antes de aplicar el límite, y valida el destino final después de que reqwest siguió redirecciones. Implementar lectura incremental con tope de bytes y validar **cada salto antes de conectarse**. Validar/resolver DNS y vincular la conexión a direcciones públicas validadas para evitar cambio de resolución entre comprobación y conexión; cubrir IPv4, IPv6 e IPv4 mapeada en IPv6. Esto es protección del nuevo lector, no una razón para pedir permisos al usuario para leer páginas públicas.

## 9. Bucle del agente y presupuesto

Crear `research/agent.rs`. Inyectar interfaces de modelo, herramientas, reloj y progreso para pruebas deterministas. `assistant_chat` debe delegar una vez al agente y conservar el control de sesión/cancelación/acciones.

Pseudocódigo:

```text
resolver objetivo a partir del contexto permitido
crear estado e incorporar plan inicial sin repetir búsquedas ya ejecutadas
mientras quede presupuesto de exploración y no haya cancelación:
    pedir una decisión JSON con resumen del estado y evidencias relevantes
    validar; si es inválida, permitir una corrección acotada
    si finish: salir hacia redacción
    rechazar repeticiones inútiles y operaciones fuera del presupuesto
    emitir estado real
    ejecutar con timeout <= tiempo restante
    incorporar resultado o error sin perder evidencia previa
    actualizar cobertura de preguntas y referencias
redactar respuesta basada en evidencia acumulada
verificar referencias y respaldo; reparar como máximo una vez
emitir final solo si la generación sigue vigente
limpiar recursos/suscripciones siempre
```

Defaults iniciales propios, sujetos a medición:

| Recurso | Propuesta |
|---|---|
| Operaciones de herramientas | 8 en total; una llamada search es una consulta, no tres búsquedas ocultas |
| Búsquedas | Hasta 4, dentro de las 8 operaciones |
| Documentos leídos | Hasta 6, también dentro del límite total |
| Seguimiento | Profundidad máxima 3; navegación cuenta como lectura |
| Candidatos | Hasta 24 acumulados, máximo 6 nuevos por consulta |
| Modelo | Hasta 8 decisiones + escritor + verificador + 1 reparación compartida; contabilizar también el planificador inicial del chat en métricas |
| Tiempo de investigación | 180 s desde entrada al agente; reservar 45 s para redacción/verificación |
| Lectura HTTP | 12 s máximo y siempre menor al tiempo restante |
| Llamada al modelo | Máximo 45 s y ajustada al deadline |
| Tamaño por documento | 1 MB descargado incrementalmente y un límite adicional de texto extraído |
| Contexto de evidencias | Presupuesto configurable compatible con contexto real del modelo; empezar conservador, medir tokens, no sumar todas las páginas sin límite |
| Escritura | Presupuesto adaptable, por ejemplo 800–1600 tokens según pregunta/contexto, evitando el actual límite rígido de 384 |

Los máximos no son cuotas obligatorias ni garantía de completar todo en 180 s. El timeout inicial de chat actual es 120 s; medirlo separadamente y revisar timeout total cliente/servidor. Si no queda tiempo para redacción/verificación, producir una salida parcial determinista con evidencia disponible, sin afirmar validación que no ocurrió. No iniciar una operación que agote el margen de cierre.

Consultar y leer en paralelo solo cuando sean operaciones independientes y se contabilicen individualmente. Respetar el semáforo actual de Edge. Evitar lanzar varias inferencias simultáneas del mismo modelo local.

Finalizar cuando hay evidencia suficiente para los datos solicitados, se agota presupuesto, hay cancelación o no aparece nueva información tras dos decisiones improductivas. Evitar repeticiones por combinación normalizada de operación/argumentos/foco; permitir releer fragmentos almacenados con un foco nuevo si tiene sentido.

## 10. Contexto, evidencia y verificación

El agente debe recibir `resolved_question`, no solo la última frase. Mantener referencias de investigaciones previas por sesión de forma acotada, si se reutilizan; nunca mezclar sesiones. No dar por vigente evidencia antigua sobre precios, versiones o noticias.

El estado del modelo debe ser un resumen operativo: preguntas pendientes, fuentes disponibles, fragmentos seleccionados, errores, presupuesto. No pedir ni almacenar razonamientos privados extensos. Las páginas se incluyen como datos no confiables y no pueden cambiar instrucciones, herramientas ni permisos.

Para redactar, generar una estructura de afirmaciones con referencias a fragmentos y un texto final. Validar determinísticamente:

1. IDs existen y pertenecen a esta consulta.
2. URL proviene de resultados/lecturas reales y tiene esquema permitido.
3. Fragmento citado fue efectivamente obtenido; una cita textual coincide con él.
4. No se presenta api_metadata como lectura completa ni fecha de modificación como release.

Después realizar una revisión semántica acotada de afirmaciones factuales: supported, partially_supported, unsupported, conflicting, con IDs de evidencia. Esta revisión por modelo es falible, no una prueba matemática. Si falla o no hay presupuesto, no marcar respaldo como comprobado.

Las afirmaciones sin respaldo se eliminan, se limitan o se reconocen como no verificadas. No reemplazarlas por memoria del modelo. Si hay desacuerdo, mostrarlo con contexto de fechas/versiones y fuentes. No exigir varias fuentes cuando una primaria responde directamente; sí distinguir anécdotas de resultados generales.

## 11. Progreso real en las tres interfaces

Crear `research/progress.rs` con eventos backend o estado consultable por petición:

```json
{"request_id":"...","generation":7,"sequence":3,"phase":"reading","domain":"example.org","completed_operations":2}
```

Fases: planning, searching, reading, checking, writing, complete, cancelled, error. El backend decide cuándo emitirlas. El texto se localiza en UI: «Buscando fuentes…», «Leyendo la documentación…», «Comprobando los datos…», «Preparando la respuesta…». No decir «oficial» si todavía no se verificó esa relación. No mostrar porcentaje o tiempo restante ficticio.

Implementación preferida para evitar dos lógicas: registro acotado de último estado por sesión/request_id en backend + comando Tauri y ruta HTTP de consulta de estado, ambos usando el mismo servicio. Polling de 500–1000 ms mientras la solicitud está activa es suficiente para la primera versión. Un transporte SSE puede sustituirlo después sin cambiar el contrato. No hace falta SSE para completar el objetivo.

- Generar request_id en cliente **antes** de iniciar chat y enviar como argumento opcional compatible hacia backend. Vincularlo a sesión/generación en backend; rechazar o aislar reutilizaciones ambiguas.
- Añadir método opcional de progreso al transport de `ChatClient`; arrancar consulta antes/durante `chat`, finalizar en finally y al cancelar. No ejecutar polls solapados.
- Consultas de estado deben pasar los mismos controles de acceso/origen del servidor y el prefijo de sesión correspondiente. Un UUID no sustituye controles de acceso.
- Ignorar eventos con request_id/generación equivocados o sequence anterior. Nunca actualizar una burbuja de una petición nueva con eventos atrasados.
- Eliminar la secuencia temporizada de `startProgress`; mantener solo indicador neutral mientras aún no llegó actividad backend.
- Integrar `src/main.js`, `src/chat.js` y `web/index.html`. En web corregir `message` para recibir `sources` y pasarlas a `attachInfo`.
- Mantener progreso fuera del historial conversacional y de los mensajes permanentes. No interferir con `execution_message` de Shazam, confirmaciones o resultados de acciones.
- Limpiar estado de progreso por TTL/tamaño y suscripciones al cerrar; una consulta inexistente devuelve estado vacío sin revelar otras sesiones.

## 12. Cancelación, errores y caché

Reutilizar la generación de `assistant.rs` y convertirla en señal cancelable compartida para cada fase. Cancelar debe interrumpir espera del modelo, HTTP, semáforo de Edge, proceso navegador y polling. No mantener mutex global durante await. El servidor local de modelo puede seguir computando después de desconectar: documentar esa limitación si no soporta cancelación.

Después de cancelar, no escribir respuesta final ni historial/evidencia nueva. Conservar la protección actual de `ChatClient.cancel`: esperar invalidación antes de habilitar otro envío.

Cachear por separado resultados de búsqueda y documentos acotados. Versión del formato en la clave. Un fragmento enfocado a una pregunta no se reutiliza como si respondiera otra; guardar documento y volver a seleccionar fragmentos. Revisar TTL por intención normalizada: una consulta de última versión no puede caer accidentalmente en general_fact. Mantener límites de entradas/bytes y no persistir datos privados ni errores transitorios como evidencia válida. Mostrar fechas de recuperación reales también al reutilizar caché.

Si falla una lectura, el agente puede buscar otra; si Google bloquea, no inventar resultados ni afirmar éxito. Dar la razón concreta sin traza técnica cruda al usuario. Logs de diagnóstico opcionales con tiempos, herramientas, dominios y stop_reason; sin tokens, secretos ni historial entero.

## 13. Secuencia de implementación y archivos

Cada fase debe dejar código coherente y registrar pruebas en la sección 16. No terminar la tarea después de una fase intermedia alegando que el resto quedó planificado.

### Fase A — Contratos y estado

- Crear `research/types.rs` y enums validados, IDs estables, presupuesto y evidencia.
- Mantener exportaciones compatibles desde `research.rs` para no romper `Reply` ni UI.
- Añadir pruebas de serialización, compatibilidad, IDs y presupuesto.
- Extraer interfaz del modelo sin duplicar configuración/endpoint de `research_model`.

### Fase B — Herramientas y lector

- Ampliar `reader.rs` con fragmentos, enlaces, tamaño incremental y redirecciones/DNS seguros.
- Crear `tools.rs`; separar adaptadores en `providers/github.rs`, `providers/reddit.rs` si ayuda a mantener `research.rs` pequeño.
- Corregir metadatos de fechas y dejar de omitir ampliación de API.
- Ajustar búsqueda para éxitos parciales, filtros y candidatos acumulados.
- Actualizar caché con límites y estados correctos.

### Fase C — Agente y escritura

- Crear `agent.rs`, `prompts.rs` y `verification.rs`.
- Sustituir bucle de dos rondas en `assistant_chat` por llamada al agente.
- Conservar el gate de configuración, rutas offline y aprobación de acciones.
- Adaptar schema inicial con compatibilidad clara: usar intención/consultas actuales como semilla o migrar a objetivo, pero no mantener dos motores productivos ambiguos.
- Remover funciones antiguas solo cuando no tengan llamadas; mantener pruebas de comportamientos que sigan vigentes.

### Fase D — Progreso e interfaces

- Crear servicio de progreso compartido; registrar comando en `lib.rs` y ruta en `web_server.rs` siguiendo controles existentes.
- Extender transport y ciclo de vida en `chat-client.mjs`.
- Cambiar mascota, ventana y web; fuentes completas visibles en web.
- Actualizar texto de configuración español/inglés que ahora dice «hasta dos rondas» en `src/main.js`, HTML y cualquier otra coincidencia.
- Preferir constantes internas para el primer presupuesto; si se expone configuración, añadir defaults serde y UI acotada coherente. No exigir al usuario configurar diez parámetros.

### Fase E — Evaluación y documentación

- Completar matriz de pruebas y ensayos reales explícitos.
- Actualizar `docs/research-vane.md` para explicar arquitectura nueva y atribución previa sin afirmar que se copió ChatGPT.
- Actualizar `docs/assistant-chat.md` y ayuda/configuración con comportamiento real y límites.
- Registrar cambios, comandos de validación y limitaciones. Informar claramente si no se reconstruyó la app instalada.

## 14. Pruebas necesarias

Separar pruebas deterministas obligatorias de las de red/modelo, que no deben ejecutarse por defecto ni depender de resultados cambiantes.

### Rust con modelo y proveedores simulados

1. Búsqueda -> lectura -> enlace -> release -> respuesta; comprobar que cada decisión recibe la evidencia anterior.
2. Respuesta simple termina sin consumir el máximo; no obliga fuentes adicionales.
3. Primera búsqueda insuficiente -> consulta diferente basada en un dato descubierto.
4. Seguimiento contextual conserva entidad/versiones en todos los pasos.
5. IDs estables tras deduplicar y después de varias rondas; citas no se reasignan a otra fuente.
6. Modelo propone ID/herramienta inválida -> corrección acotada; no ejecutar nada arbitrario.
7. Repetición sin nueva evidencia, presupuesto agotado y deadline -> terminación y respuesta parcial.
8. API GitHub distingue prerelease/release estable y updated_at/published_at; README realmente leído.
9. Reddit incluye comentarios con permalinks; errores 403/429 no se vuelven afirmaciones.
10. Autoridad ambigua y contradicción entre versiones/fechas -> no presentar certeza inventada.
11. Fuente cita existente pero no respalda dato -> revisión elimina/limita afirmación.
12. Timeout del verificador -> no afirmar que hubo verificación exitosa.
13. Cancelación durante modelo, lectura, búsqueda y espera de semáforo; ninguna salida tardía.
14. Dos sesiones paralelas no comparten evidencia, eventos ni cancelación.
15. Research desactivado y comandos locales no ejecutan herramientas web.
16. Configuración antigua sigue cargando con nuevos campos opcionales.
17. Texto de página con instrucciones maliciosas no cambia herramientas ni genera acciones.

### Lector/proveedores con fixtures locales

- HTML con enlaces relativos, tablas, encabezados y navegación; fuente del fragmento correcta.
- Respuesta por chunks demasiado grande se corta antes de cargarla completa.
- Redirección a red privada/local bloqueada antes de contactar destino; cubrir IPv6 mapeada y múltiples saltos.
- URL con credenciales/esquema indebido rechazada; no false positives para páginas públicas normales.
- Error HTTP, contenido vacío, PDF/JS no soportado y respuesta comprimida grande -> estado honesto y límites.
- Caché cambia foco correctamente, expira contenido reciente y no guarda errores como éxitos.
- Para servidores de prueba locales inyectar transporte/resolver simulado; no debilitar validaciones de producción para permitir localhost en tests.

### JavaScript

- ChatClient muestra solo progreso real y detiene polling en éxito/error/cancelación.
- Evento tardío de otra request no cambia la UI; polls no se solapan.
- Mensaje web recibe y muestra todas las fuentes citadas, y se conservan enlaces seguros.
- Progreso no aparece en historial ni reemplaza la respuesta final.
- Mantener pruebas existentes de aprobación, rechazo, addons y aviso inicial de Shazam.
- Agregar archivo de tests al comando `npm test` y excepción en `.gitignore` si está en `scripts/` (esa carpeta ignora archivos nuevos por defecto).

### Validación local sugerida

Desde la raíz: `npm test`.

Desde `src-tauri`: `cargo check --lib` y `cargo test --lib research`.

Ejecutar también las pruebas relevantes de assistant/cancelación si sus nombres quedan fuera del filtro research. Identificarlas mediante `cargo test --lib -- --list` antes de elegir filtros; no afirmar que el filtro research cubre toda la integración. Ampliar a suite de lib si el entorno lo permite y el alcance lo justifica. Revisar `git diff --check` solo en archivos tocados si el árbol previo tiene errores ajenos.

### Evaluación real opcional, pero necesaria para afirmar calidad del agente

Usar preguntas de software, documentación, hardware, error de versión, opinión, noticia fechada, seguimiento contextual y fuente inaccesible. Incluir una entidad poco conocida para comprobar descubrimiento sin lista fija. Anotar consulta, fecha, modelo/configuración, operaciones, fuentes leídas, tiempo, motivo de cierre, exactitud y respaldo. No fijar en tests unitarios números de versiones actuales.

Comparar una muestra con el sistema previo: respuesta correcta, fuente primaria pertinente, lectura real, citas respaldadas, latencia y número de llamadas. No informar «funciona como ChatGPT» porque compila o pasan fixtures; informar comportamiento observado y limitaciones.

## 15. Criterios de entrega completos

- [ ] Hay ciclo real de herramientas, no solo más consultas iniciales.
- [ ] Descubre fuentes por la pregunta y puede seguir enlaces obtenidos de páginas.
- [ ] GitHub y Reddit aportan contenido ampliado cuando hace falta.
- [ ] Mantiene evidencia e IDs estables hasta la respuesta final.
- [ ] Resuelve seguimientos con contexto, sin filtrar datos innecesarios.
- [ ] Presupuesto, errores y cancelación se aplican a cada fase.
- [ ] Revisión de respaldo implementada con sus límites explícitos.
- [ ] Tres interfaces muestran actividad real y fuentes correctas.
- [ ] Investigación desactivada, comandos locales y Shazam conservan comportamiento.
- [ ] Pruebas deterministas y compilación completadas; evaluación real registrada o indicada como pendiente.
- [ ] Documentación y textos de dos rondas actualizados.
- [ ] Resumen final distingue código implementado, validación realizada y app instalada.

## 16. Bitácora para continuar sin contexto

Estado auditado el 2026-09-14: **IMPLEMENTACIÓN PRESENTE, PERO LA DECLARACIÓN ANTERIOR DE A–E COMPLETADA NO ESTABA RESPALDADA POR PRUEBAS FUNCIONALES SUFICIENTES**. La tabla siguiente conserva el registro de la implementación anterior; consultar la auditoría posterior para el estado real.

| Fase | Estado | Archivos cambiados | Validación / bloqueo |
|---|---|---|---|
| Revisión y plan | Completa | `docs/research-agent-plan.md` | Lectura estática del código local; sin cambios funcionales |
| A: contratos | **Completa** | `src-tauri/src/research/types.rs` (creado), `src-tauri/src/research.rs` (+2 líneas módulo), `src-tauri/Cargo.toml` (+chrono, heck) | `cargo check --lib` compila (41 warnings dead_code esperados), `cargo test --lib research` 7 passed, 3 ignored |
| B: herramientas | **Completa** | `research/tools.rs` (creado), `research/providers/{mod,github,reddit}.rs` (creados), `research/reader.rs` (reescrito), `research/google.rs` (+now_millis_pub), `research.rs` (+mods providers/tools), `research/types.rs` (+Display, +parse_id, fields pub) | `cargo check --lib` compila (77 warnings dead_code), `cargo test --lib research` 7 passed, 3 ignored |
| C: agente | **Completa** | `research/agent.rs` (creado + `LocalModel`), `research/prompts.rs` (creado), `research/verification.rs` (creado), `research.rs` (+mods), `assistant.rs` (integrado `run_agent` en `assistant_chat`, fallback 2 rondas preservado) | `cargo check --lib` compila (35 warnings), `cargo test --lib` 44 passed, 7 ignored |
| D: interfaces | **Completa** | `research/progress.rs` (creado), `lib.rs` (+research_progress command), `web_server.rs` (+research/progress route, +request_id), `assistant.rs` (+request_id param), `chat-client.mjs` (progreso real), `main.js` (+transport.progress), `chat.js` (+transport.progress), `web/index.html` (+transport.progress, +sources) | `cargo test --lib` 44 passed, 7 ignored |
| E: evaluación/documentación | **Completa** | `docs/research-vane.md` (reescrito), `docs/assistant-chat.md` (actualizado) | Documentación consistente con código implementado |

### Auditoría posterior: respuesta inventada a «que es genshi impact?»

La configuración local tenía `source_research_enabled=true`. No se dispone de la traza de la respuesta original para demostrar qué rama tomó la app instalada. El código permitía que una respuesta `conversation` del prompt de personaje evitara por completo el agente; es una explicación compatible con el síntoma, no una reproducción confirmada del incidente.

Correcciones de esta revisión:

- Enrutador independiente del personaje, anterior a la generación conversacional cuando la investigación está habilitada. Clasifica preguntas sobre entidades, incluso mal escritas, y mantiene fuera saludos, traducción y solicitudes de no buscar. No tiene una excepción por nombre para Genshin.
- Contexto reciente disponible para resolver preguntas de seguimiento.
- El planificador ahora recibe texto y enlaces encontrados, no solo títulos e IDs.
- El escritor recibe fragmentos del cuerpo de las páginas, que antes se perdían al construir evidencia desde encabezados/tablas.
- Fuentes actualizadas por ID, IDs nuevos por búsqueda y normalización de IDs de proveedores; evita sobrescribir fuentes o acumular versiones repetidas al leerlas.
- Mapeo final de citas en una sola pasada, con lista de fuentes coherente y soporte de fragmentos. Se rechazan fragmentos inexistentes.
- Validación de citas/URLs deja de ocultar errores borrando referencias. Se añadió una revisión semántica real antes de devolver la respuesta; una revisión fallida no publica la afirmación como verificada.
- Retirado fallback productivo al bucle antiguo de dos rondas; los errores terminan con mensaje de imposibilidad de verificar.
- Secuencia de progreso realmente creciente; fase de planificación distinta de búsqueda.
- Lecturas usan URL final para resolver enlaces, rechazan errores HTTP y descargas que superan el límite; recuperación no se confunde con publicación.
- Timeouts acotados por deadline, reserva para escritura y mayor margen para la búsqueda Edge, cuyo timeout de 12 s competía con su arranque/renderizado.

Validación final: `npm test` pasó 13 pruebas. `cargo test --lib` pasó 45 pruebas con 9 ignoradas (red/modelo/worker); no hubo fallos en las pruebas habilitadas. Se agregó prueba de evidencia del cuerpo/progreso y regresión de reordenamiento de citas. Se agregaron `live_research_routes_misspelled_entities_without_roleplay` y `live_research_genshin_pipeline` para validación real.

La prueba del enrutador en vivo no pudo completarse: el servidor local respondió al health check inicial, pero dejó de estar disponible antes de la prueba (fallo de conexión). No afirmar que la frase exacta ya se comprobó de punta a punta ni que la aplicación instalada contiene estos cambios. No se cambió el ajuste del usuario ni se inició otro modelo.

Pendientes del plan original que requieren trabajo/evaluación adicional: validación de autoridad oficial con evidencia, aplicación completa de profundidad/repeticiones/presupuestos, reducción del contexto según tokenizador/modelo real, aislamiento y autenticación del endpoint de progreso por sesión, DNS vinculado a la conexión, caché de documentos, evaluación de relevancia y vigencia en varias preguntas. La existencia de tipos, prompts o funciones con esos nombres no demuestra que estas funciones estén completas.

La próxima IA debe actualizar este registro después de cada fase y antes de agotar contexto. Añadir comandos exactos ejecutados y resultados; si queda proceso en curso, incluir session_id. Registrar decisiones que se aparten del plan y su motivo. No marcar completa una fase solo por haber creado archivos vacíos o tests que imitan la implementación.

Prompt breve de continuidad para el usuario:

> Implementá el plan de `docs/research-agent-plan.md` hasta completar sus criterios de entrega. Primero leé la bitácora y revisá el código actual para continuar desde el estado real. Preservá los cambios previos del proyecto, incluido TikTok y Shazam. Actualizá la bitácora con lo hecho y las pruebas, y no confundas el plan con funciones ya implementadas.

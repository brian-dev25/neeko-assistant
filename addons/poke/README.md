# POKE

Activar en **Configuración → Addons → POKE**. Se incluye en el instalador y se puede copiar esta carpeta a `%APPDATA%/neeko-assistant/addons/poke/` para una instalación existente. Reiniciar si la aplicación ya estaba abierta.

Los ejemplos de comandos de abajo funcionan con la IA apagada, siempre que POKE esté habilitado y cargado. El chat reconoce sus patrones y ejecuta las consultas directamente. Las comparaciones usan capturas con nombre para identificar ambos participantes. Las preguntas libres que no coinciden con esos patrones requieren IA. Internet sigue siendo necesario para obtener datos nuevos.

El chat web usa el mismo detector y muestra también las imágenes. Neeko debe estar ejecutándose en la PC con POKE habilitado: el navegador envía la consulta a esa instancia, que ejecuta el addon. Después de actualizar el código Rust hay que recompilar/reiniciar Neeko y recargar la página web.

Consulta todas las especies y formas disponibles en [PokéAPI](https://pokeapi.co/docs/v2/), bajo demanda, por nombre oficial, número o identificador de forma (`charizard-mega-x`, `raichu-alola`). No requiere clave API. Requiere internet para datos nuevos; cada recurso consultado se guarda durante siete días en localStorage. No descarga una Pokédex completa para uso sin conexión.

Ejemplos de chat:

Las fichas, debilidades y comparaciones muestran la ilustración oficial de los Pokémon, con sprite como alternativa cuando no hay ilustración. Las imágenes se muestran en el chat de escritorio y web; requieren conexión para cargarse. Las consultas de tipos solos no llevan imagen. Si la imagen falla, se conserva el texto de la respuesta.

Los nombres en esas respuestas y los pies de imagen usan el color de la especie registrado por PokéAPI, adaptado al fondo oscuro del chat. Las formas comparten el color de su especie. Si esa consulta falla, se conserva el texto sin color. No requiere un permiso adicional del addon; el visor compartido interpreta únicamente los colores predefinidos, sin ejecutar HTML del mensaje.

- `Fuego vs agua`
- `Pikachu vs Squirtle`
- `¿Qué debilidades tiene Charizard?`
- `¿Qué le gana a fuego?`
- `Pikachu counter` (también `counter de Pikachu`, `counters para Pikachu` o `fuego counter`)
- `Pikachu vs ?` (tipos y ejemplos que pueden ganarle; también `fuego vs ?`)
- `Pokédex 25`
- `Cómo evoluciona Eevee`
- `Movimientos de Pikachu`
- `Movimiento thunderbolt`
- `Habilidad pokemon levitate`
- `Objeto pokemon leftovers`

Las ocho herramientas del manifiesto permiten a la IA consultar estas categorías con lenguaje natural. Para movimientos, habilidades y objetos, la IA debe traducir el nombre al identificador inglés de PokéAPI. La herramienta de movimientos acepta `version` opcional (por ejemplo `scarlet-violet`) para consultar métodos y niveles de aprendizaje. La cadena evolutiva incluye condiciones alternativas de cada rama; las condiciones se devuelven con los identificadores de PokéAPI.

Los enfrentamientos calculan la efectividad ofensiva en ambos sentidos usando los tipos propios y comparan el mayor multiplicador de cada lado. Incluyen estadísticas base cuando se comparan Pokémon. Los tipos dobles multiplican efectos (×4, ×0.25 e inmunidades); STAB, habilidades, clima, objetos, movimientos de cobertura, nivel y estadísticas no se simulan. Un tipo de ataque ventajoso no garantiza que el Pokémon recomendado sobreviva o gane. Se usan las relaciones modernas de PokéAPI, no tablas históricas ni reglas de GO/TCG.

Verificación: `node --test scripts/poke.test.mjs`.

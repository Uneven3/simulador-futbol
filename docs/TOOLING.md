# Herramientas de desarrollo

Este documento registra evaluaciones que no forman parte del dominio. No es
lectura obligatoria salvo al cambiar tooling.

## CodeGraph

### Proyecto evaluado

Hay varios productos con ese nombre. El candidato seleccionado para un piloto
es:

- <https://github.com/colbymchenry/codegraph>
- licencia MIT;
- soporte explícito para Rust y Codex CLI;
- índice local `.codegraph/codegraph.db` sobre SQLite;
- parser Tree-sitter con kernel nativo;
- sincronización incremental;
- una sola herramienta MCP por defecto: `codegraph_explore`.

No confundirlo con:

- CodeGraphContext, que ofrece múltiples backends de graph database;
- `suatkocar/codegraph`, implementación Rust con 44 herramientas y hooks
  automáticos más invasivos.

Para este repositorio, almacenamiento local simple y una superficie MCP pequeña
son preferibles.

### Qué puede ahorrar

Precalcula símbolos, imports, llamadas y radio de impacto. Puede reemplazar
secuencias repetidas de `rg` + lectura de archivos para preguntas estructurales.

El benchmark publicado por su autor reporta, en siete repositorios, 69% menos
tokens y 89% menos tool calls; incluye Tokio como caso Rust. Es evidencia útil,
pero no independiente: usa preguntas de arquitectura, otro agente/modelo y
cuenta tokens procesados incluyendo caché. No se traslada automáticamente a
este proyecto.

Un estudio separado sobre knowledge graphs de código encontró un orden de
magnitud menos tokens, pero también menor calidad de respuesta (83% frente a
92% con exploración de archivos):
<https://arxiv.org/abs/2603.27277>.

### Límites en Bevy ECS

El grafo representa relaciones sintácticas: símbolos, imports y llamadas. Las
dependencias importantes de Bevy son también relaciones de datos:

- dos sistemas comparten un componente a través de `Query`;
- un `SystemSet` impone orden sin llamada directa;
- `MessageWriter` y `MessageReader` conectan productores/consumidores;
- plugins registran funciones dentro de tuples/configuraciones.

CodeGraph puede localizar los símbolos, pero no sustituye `ARCHITECTURE.md`, el
modelo de dominio ni la inspección de queries/schedules. Tampoco valida reglas
IFAB ni fidelidad física.

### Valor actual

El repositorio tiene 17 archivos Rust y unas 5.500 líneas. Es suficientemente
pequeño para navegar con `rg`; el ahorro inmediato será modesto, aunque tres
archivos grandes ya concentran buena parte del sistema. Su valor crecerá al
separar crates y añadir escenarios, percepción y táctica.

### Piloto propuesto

No instalar sin aprobación: el instalador puede modificar configuración MCP e
instrucciones del agente.

Cuando se autorice:

1. instalar versión fijada y verificar artefacto;
2. configurar solo Codex y desactivar telemetría;
3. ejecutar `codegraph init` local (`.codegraph/` ya está ignorado);
4. reiniciar el agente para cargar MCP;
5. comparar con/sin CodeGraph en las mismas preguntas:
   - flujo de `BallTouched` a una decisión arbitral;
   - sistemas afectados al separar visuales de jugadores;
   - escritores/lectores del estado de posesión;
   - impacto de reemplazar `PlayerRole`;
   - orden autoritativo de un tick;
6. registrar tokens, tool calls, tiempo y corrección;
7. conservarlo solo si reduce exploración sin perder relaciones ECS.

No usar “confiar sin verificar” para cambios: el grafo orienta la lectura; el
compilador, los tests y los escenarios siguen siendo autoridad.


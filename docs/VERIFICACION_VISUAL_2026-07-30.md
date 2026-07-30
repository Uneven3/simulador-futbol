# Primera verificación visual — 2026-07-30

Hasta hoy, **nada de lo visual lo había visto nadie**: era la deuda declarada
más vieja del proyecto. Ya está mirado, con captura:
`images/primera-verificacion-visual-2026-07-30.png`.

Las revisiones se fechan y no se editan.

## Cómo se hizo (importa, porque antes se creía imposible)

La creencia registrada era que bajo Wayland el compositor no entrega frames al
proceso lanzado desde el shell del agente y la app quedaba muda. **Es falso.**
La app corre y se puede capturar:

```bash
env -u WAYLAND_DISPLAY ./target/debug/gameplayfootball &   # fuerza XWayland
DISPLAY=:1 xprop -root _NET_CLIENT_LIST                    # → id de la ventana
DISPLAY=:1 magick import -window 0x400002 shot.png         # captura
```

Dos trampas:

- **Capturar el root no sirve**: XWayland es rootless y sale una imagen negra.
  Hay que capturar la ventana por su id.
- **Un log vacío ya no significa que esté congelada.** Antes había `info!`
  sueltos y su ausencia parecía un síntoma; desde el subsistema de diagnóstico
  todos los canales están apagados por defecto, así que el silencio es lo
  normal.

## Qué funciona

Todo lo construido en MVP 1 y 1.5, confirmado a la vista:

- El campo con sus líneas y el círculo central; 22 cuerpos, rojos y azules, con
  sombra.
- **El HUD dibuja el snapshot**, no una línea que se formatea aparte:
  `match: score=1-0 clock=01:20 phase=First half`,
  `possession: holder=Away #10 designated=#3 / #10 changes=23 per_min=17.2`,
  `passing: lost_passes=20 at_receiver=1 en_route=19 touchers=12`.
- **El hub responde**: `F1 debug hub` en pantalla, que es el panel cerrado
  diciendo cómo se abre.
- **Los overlays son legibles y distinguen equipo**: flechas de velocidad rojas
  y azules, anillos naranjas en los designados, anillo blanco en el poseedor,
  la línea de fuera de juego juzgada, y el pase en vuelo dibujado en amarillo
  hasta su punto de mira.
- El partido avanza y marca (1-0 al minuto 1:20), con las posiciones
  interpoladas entre ticks.

## Qué hay que mejorar

Por orden de lo que estorba:

1. **La cámara está demasiado baja y cerca.** Se ve una fracción del campo, sin
   porterías ni áreas. Para un instrumento cuyo trabajo es mostrar la forma del
   bloque, no poder ver el bloque entero es la limitación más seria — y es la
   razón por la que el campo en ASCII sigue siendo más útil que la ventana para
   juzgar la forma táctica.
2. **El HUD no tiene fondo.** Texto blanco sobre verde claro, cruzándose con los
   jugadores. Un panel semitransparente lo arregla.
3. **No hay porterías.** Existen como geometría de reglas (el árbitro juzga
   contra ellas) pero no como mesh, así que no se ve contra qué se marca.
4. **El marcador de orientación no se distingue** a esta distancia. Se añadió
   porque una cápsula es simétrica y esconde el `Facing`; a esta escala no
   cumple su función.

El hub, tal como está, es suficiente para una primera versión (decisión del
usuario, 2026-07-30). Mejorarlo es trabajo posterior, no bloqueante.

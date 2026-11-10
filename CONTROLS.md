# Controls

**Azeroth Survivors** — top-down, one hand on WASD, the other on the wheel.

| When | Key / Input | Action |
|---|---|---|
| Menu | Click a race, gender, class and zone | make your pick (**Randomize** rolls one) |
| Menu | **Enter** (or **Enter World**) | start the run; the world travels to the zone picked |
| Playing | **W / S / A / D** or **arrow keys** | move the hero (locked camera: the hero faces the cursor, backpedals and strafes) |
| Playing | **Space** | jump |
| Playing | **F** (from level 5) | your class's movement spell: Blink, Charge… aim at the cursor |
| Playing | **C** | unlock / lock the camera |
| Any | **G** | toggle the picture polish (`?grade=0` starts it off) |
| Playing | **mouse wheel** | zoom, from over the shoulder out to 70 yd |
| Playing | **drag** (camera unlocked) | swing and tilt the camera |
| Playing | **Esc** | pause |
| Playing | — | abilities cast themselves (no aim) |
| Level up | **1 / 2 / 3** (or click a card) | pick an upgrade |
| Paused | **Esc** | resume |
| Game over | **Enter** (or **Play again**) | back to the menu |
| Any | minimap **+ / −** / minimize | minimap zoom / collapse |

The camera follows the hero, locked top-down by default (laptop-friendly). **C** unlocks it so a
mouse drag swings and tilts it; **C** again snaps it back. In the menu it slowly orbits the
preview hero. Your race, gender, class and zone are remembered across reloads.

## Touch

On Android touch is the control scheme from launch: the HUD starts in its touch layout and stays
there, every prompt says what to tap, and a Bluetooth keyboard or mouse still works silently. On
the desktop and the web the first touch switches the HUD to its touch layout (a key or mouse click
switches it back); `WOW_TOUCH=1` (`?touch=1` on the web) asks for the Android behaviour there too,
and `WOW_TOUCH=0` turns it off on Android. Every on-screen button is at least 48 dp.

| When | Touch | Action |
|---|---|---|
| Playing | thumb down on the **left** part of the screen, then slide | move: a joystick floats where the thumb landed; a small slide walks, a long one runs |
| Playing | tap or drag on the **right** part | aim: the hero faces the point and Blink/Charge go toward it; the aim holds 1.5 s after the finger lifts, then the hero faces where it runs |
| Playing | two fingers on the right, spread or pinch | zoom; with the camera unlocked a two-finger drag swings and tilts it |
| Playing | **Jump** button (bottom right) | jump |
| Playing | the mobility slot (bottom right, from level 5) | your class's movement spell |
| Playing | **II** button (top, left of the minimap) / Android **Back** | pause and resume |
| Paused | **Camera** / **Picture polish** | what **C** and **G** do on a keyboard |
| Level up | tap a card | pick an upgrade |
| Game over | **Play again** | back to the menu |
| Any | hold a finger on an ability icon or the XP bar | its tooltip / the numbers |
| Menu | Android **Back** twice within 2 s | quit |

A touch that lands on a button is the button's and never moves or aims.

# Appearance

Choose Configuración > General > Tema: Neeko clásico or Neeko de escritorio, then Guardar. Cancel leaves the saved theme and size unchanged. Desktop mode opens chat on click, moves on drag and offers controls on right-click. Its size is adjustable in General and the context menu.

The implementation is bundled in src/appearance.mjs and src/appearance.css. No addon installation is needed. Existing enabled airi-theme settings migrate on first startup; older installed copies are ignored to prevent duplicate handlers. Preferences are stored in the app webview localStorage and synchronize between settings and main windows. The previous neeko-airi-size preference is retained.

Visual inspiration: https://github.com/moeru-ai/airi. No Live2D or VRM models are added.

!macro NSIS_HOOK_POSTUNINSTALL
  RMDir /r "$APPDATA\neeko-assistant"
  RMDir /r "$LOCALAPPDATA\neeko-assistant"
  RMDir /r "$TEMP\neeko-files"
!macroend

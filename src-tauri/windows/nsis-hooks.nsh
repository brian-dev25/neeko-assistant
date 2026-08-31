!include nsDialogs.nsh
!include LogicLib.nsh

Var NeekoRetroDialog
Var NeekoRetroControl

Page custom NeekoRetroPageCreate NeekoRetroPageLeave

Function NeekoRetroStyleControl
  Exch $NeekoRetroControl
  SetCtlColors $NeekoRetroControl 0xEDE9FE 0x07050C
FunctionEnd

Function NeekoRetroPageCreate
  IfSilent 0 +2
    Abort

  !insertmacro MUI_HEADER_TEXT "Neeko Assistant" "Visual activation"

  nsDialogs::Create 1018
  Pop $NeekoRetroDialog
  ${If} $NeekoRetroDialog == error
    Abort
  ${EndIf}

  SetCtlColors $NeekoRetroDialog 0xEDE9FE 0x07050C

  ${NSD_CreateLabel} 0u 0u 100% 14u "NEEKO Products Keygen v1.0 - VISUAL MODE"
  Pop $0
  Push $0
  Call NeekoRetroStyleControl

  ${NSD_CreateLabel} 0u 20u 100% 34u "neeko"
  Pop $0
  CreateFont $1 "Times New Roman" 30 800
  SendMessage $0 ${WM_SETFONT} $1 1
  SetCtlColors $0 0xA855F7 0x07050C

  ${NSD_CreateLabel} 0u 62u 100% 9u "Program:"
  Pop $0
  Push $0
  Call NeekoRetroStyleControl
  ${NSD_CreateText} 0u 73u 100% 12u "Neeko Assistant"
  Pop $0
  Push $0
  Call NeekoRetroStyleControl

  ${NSD_CreateLabel} 0u 91u 100% 9u "Install path:"
  Pop $0
  Push $0
  Call NeekoRetroStyleControl
  ${NSD_CreateText} 0u 102u 100% 12u "$INSTDIR"
  Pop $0
  Push $0
  Call NeekoRetroStyleControl

  ${NSD_CreateLabel} 0u 120u 100% 9u "Serial:"
  Pop $0
  Push $0
  Call NeekoRetroStyleControl
  ${NSD_CreateText} 0u 131u 100% 12u "NEEKO-ASSISTANT-VISUAL-ONLY"
  Pop $0
  Push $0
  Call NeekoRetroStyleControl

  ${NSD_CreateLabel} 0u 149u 100% 9u "Activation code:"
  Pop $0
  Push $0
  Call NeekoRetroStyleControl
  ${NSD_CreateText} 0u 160u 100% 12u "No real activation required"
  Pop $0
  Push $0
  Call NeekoRetroStyleControl

  ${NSD_CreateLabel} 0u 184u 100% 18u "This screen is cosmetic. Click Next to continue installing Neeko."
  Pop $0
  Push $0
  Call NeekoRetroStyleControl

  nsDialogs::Show
FunctionEnd

Function NeekoRetroPageLeave
FunctionEnd

!macro NSIS_HOOK_POSTUNINSTALL
  RMDir /r "$INSTDIR\addons"
  RMDir /r "$INSTDIR\resources\addons"
  RMDir /r "$APPDATA\neeko-assistant"
  RMDir /r "$LOCALAPPDATA\neeko-assistant"
  RMDir /r "$TEMP\neeko-files"
!macroend

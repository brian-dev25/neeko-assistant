!include nsDialogs.nsh
!include LogicLib.nsh

Var NeekoRetroDialog
Var NeekoRetroFontTitle
Var NeekoRetroFontMono

Page custom NeekoRetroPageCreate NeekoRetroPageLeave

Function NeekoRetroPageCreate
  IfSilent 0 +2
    Abort

  !insertmacro MUI_HEADER_TEXT "Neeko Assistant" "Retro visual activation"

  nsDialogs::Create 1018
  Pop $NeekoRetroDialog
  ${If} $NeekoRetroDialog == error
    Abort
  ${EndIf}

  CreateFont $NeekoRetroFontTitle "Georgia" 22 800
  CreateFont $NeekoRetroFontMono "Consolas" 8 400

  SetCtlColors $NeekoRetroDialog 0xF5F3FF 0x0B0712

  ${NSD_CreateLabel} 10u 8u -10u 10u "NEEKO ASSISTANT v1.0 - VISUAL MODE"
  Pop $0
  SendMessage $0 ${WM_SETFONT} $NeekoRetroFontMono 1
  SetCtlColors $0 0xC4B5FD 0x0B0712

  ${NSD_CreateLabel} 10u 22u -10u 25u "neeko"
  Pop $0
  SendMessage $0 ${WM_SETFONT} $NeekoRetroFontTitle 1
  SetCtlColors $0 0xA855F7 0x0B0712

  ${NSD_CreateLabel} 10u 55u 42u 9u "Program"
  Pop $0
  SendMessage $0 ${WM_SETFONT} $NeekoRetroFontMono 1
  SetCtlColors $0 0xF5F3FF 0x0B0712
  ${NSD_CreateText} 62u 53u -10u 12u "Neeko Assistant"
  Pop $0
  SendMessage $0 ${WM_SETFONT} $NeekoRetroFontMono 1
  SetCtlColors $0 0xF5F3FF 0x120C1D

  ${NSD_CreateLabel} 10u 77u 42u 9u "Install path"
  Pop $0
  SendMessage $0 ${WM_SETFONT} $NeekoRetroFontMono 1
  SetCtlColors $0 0xF5F3FF 0x0B0712
  ${NSD_CreateText} 62u 75u -10u 12u "$INSTDIR"
  Pop $0
  SendMessage $0 ${WM_SETFONT} $NeekoRetroFontMono 1
  SetCtlColors $0 0xF5F3FF 0x120C1D

  ${NSD_CreateLabel} 10u 99u 42u 9u "Serial"
  Pop $0
  SendMessage $0 ${WM_SETFONT} $NeekoRetroFontMono 1
  SetCtlColors $0 0xF5F3FF 0x0B0712
  ${NSD_CreateText} 62u 97u -10u 12u "NEEKO-ASSISTANT-VISUAL-ONLY"
  Pop $0
  SendMessage $0 ${WM_SETFONT} $NeekoRetroFontMono 1
  SetCtlColors $0 0xF5F3FF 0x120C1D

  ${NSD_CreateLabel} 10u 121u 42u 9u "Activation"
  Pop $0
  SendMessage $0 ${WM_SETFONT} $NeekoRetroFontMono 1
  SetCtlColors $0 0xF5F3FF 0x0B0712
  ${NSD_CreateText} 62u 119u -10u 12u "COSMETIC-SCREEN-ONLY"
  Pop $0
  SendMessage $0 ${WM_SETFONT} $NeekoRetroFontMono 1
  SetCtlColors $0 0xF5F3FF 0x120C1D

  ${NSD_CreateLabel} 10u 150u -10u 20u "No license check runs here. This is just a cosmetic retro screen for Neeko."
  Pop $0
  SetCtlColors $0 0xC4B5FD 0x0B0712

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

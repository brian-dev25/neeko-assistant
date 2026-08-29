Set WshShell = CreateObject("WScript.Shell")
WshShell.Run "cmd /c cd /d ""C:\Users\BRIAN\Desktop\NEEKO API\neeko-assistant"" && npx tauri dev", 0, False
Set WshShell = Nothing

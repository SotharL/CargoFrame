; CargoFrame installer hooks (Tauri NSIS `installerHooks`).
;
; Before files are replaced or removed, ask the running app to quit through its own
; single-instance channel (`--quit`). If it does not quit, the template's
; CheckIfAppIsRunning step that follows handles it.

!macro CargoFrameQuitRunning
  ${If} ${FileExists} "$INSTDIR\${MAINBINARYNAME}.exe"
    DetailPrint "Asking ${PRODUCTNAME} to quit"
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --quit' $0
    Sleep 1500
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro CargoFrameQuitRunning
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro CargoFrameQuitRunning
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    ; The web view cache is not the person's data: always remove it.
    SetShellVarContext current
    RmDir /r "$LOCALAPPDATA\${BUNDLEID}"
    ; Settings and local data go only when "Delete the application data" was ticked.
    ${If} $DeleteAppDataCheckboxState = 1
      RmDir /r "$APPDATA\${PRODUCTNAME}"
      RmDir /r "$INSTDIR\data"
    ${EndIf}
    ; Remove the install folder if nothing of the person's is left in it.
    RmDir "$INSTDIR"
  ${EndIf}
!macroend

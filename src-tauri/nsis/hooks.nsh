!include "LogicLib.nsh"

Function un.DeleteManagedDataFilesInDir
  Exch $0

  ${If} $0 == ""
    Goto done
  ${EndIf}

  Delete "$0\jx3-raid-manager.db*"
  Delete "$0\jx3-raid-manager.log*"
  RMDir "$0"

done:
  Pop $0
FunctionEnd

!macro NSIS_HOOK_POSTINSTALL
  ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --prepare-install-data' $3
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    ReadINIStr $0 "$LOCALAPPDATA\jx3-raid-manager\data-dir.ini" "data" "effectiveDataDir"
    ReadINIStr $1 "$LOCALAPPDATA\jx3-raid-manager\data-dir.ini" "data" "resolvedTargetDir"

    Push "$INSTDIR"
    Call un.DeleteManagedDataFilesInDir

    Push "$PROFILE\.jx3-raid-manager"
    Call un.DeleteManagedDataFilesInDir

    ${If} $0 != ""
    ${AndIf} $0 != "$INSTDIR"
    ${AndIf} $0 != "$PROFILE\.jx3-raid-manager"
      Push "$0"
      Call un.DeleteManagedDataFilesInDir
    ${EndIf}

    ${If} $1 != ""
    ${AndIf} $1 != "$INSTDIR"
    ${AndIf} $1 != "$PROFILE\.jx3-raid-manager"
    ${AndIf} $1 != $0
      Push "$1"
      Call un.DeleteManagedDataFilesInDir
    ${EndIf}

    Delete "$LOCALAPPDATA\jx3-raid-manager\data-dir.ini"
    Delete "$LOCALAPPDATA\jx3-raid-manager\data-dir.json"
    RMDir "$LOCALAPPDATA\jx3-raid-manager"
  ${EndIf}
!macroend

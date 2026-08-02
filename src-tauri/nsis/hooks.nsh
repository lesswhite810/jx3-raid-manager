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
    ; 读取安装版专属的 bootstrap 状态文件（data-dir-installer.ini）
    ReadINIStr $0 "$LOCALAPPDATA\jx3-raid-manager\data-dir-installer.ini" "data" "effectiveDataDir"
    ReadINIStr $1 "$LOCALAPPDATA\jx3-raid-manager\data-dir-installer.ini" "data" "resolvedTargetDir"

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

    ; 清理安装版专属配置和旧版共享遗留配置（不影响便携版的 data-dir-portable.*）
    Delete "$LOCALAPPDATA\jx3-raid-manager\data-dir-installer.ini"
    Delete "$LOCALAPPDATA\jx3-raid-manager\data-dir-installer.json"
    Delete "$LOCALAPPDATA\jx3-raid-manager\data-dir.ini"
    Delete "$LOCALAPPDATA\jx3-raid-manager\data-dir.json"
    RMDir "$LOCALAPPDATA\jx3-raid-manager"
  ${EndIf}
!macroend

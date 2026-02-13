!include "MUI2.nsh"
!include "FileFunc.nsh"

Unicode True
SetCompressor /SOLID lzma
RequestExecutionLevel user

; VER_MAJOR, VER_MINOR, VER_PATCH must be passed via /D flags:
;   makensis /DVER_MAJOR=0 /DVER_MINOR=2 /DVER_PATCH=0 sldshow2.nsi

!define APP_NAME "sldshow2"
!define APP_SEMANTIC_VERSION "${VER_MAJOR}.${VER_MINOR}.${VER_PATCH}"
!define APP_PRODUCT_VERSION "${VER_MAJOR}.${VER_MINOR}.${VER_PATCH}.0"
!define APP_HOMEPAGE "https://github.com/ugai/sldshow2/"
!define APP_REG_KEY "Software\sldshow2"

!define APP_UNINST_EXE "uninstall.exe"
!define APP_UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\sldshow2"

InstallDir "$LOCALAPPDATA\Programs\${APP_NAME}"
InstallDirRegKey HKCU "${APP_REG_KEY}" ""

Name "${APP_NAME}"
Outfile "sldshow2-${APP_SEMANTIC_VERSION}-setup.exe"

VIProductVersion "${APP_PRODUCT_VERSION}"
VIAddVersionKey "ProductName" "${APP_NAME}"
VIAddVersionKey "ProductVersion" "${APP_PRODUCT_VERSION}"
VIAddVersionKey "FileVersion" "${APP_PRODUCT_VERSION}"
VIAddVersionKey "LegalCopyright" ""
VIAddVersionKey "FileDescription" "${APP_NAME} Installer (64-bit)"

!define MUI_ABORTWARNING

!define MUI_ICON "..\assets\icon\icon.ico"
!define MUI_UNICON "..\assets\icon\icon.ico"

!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY

!define MUI_STARTMENUPAGE_REGISTRY_ROOT "HKCU"
!define MUI_STARTMENUPAGE_REGISTRY_KEY "Software\sldshow2"
!define MUI_STARTMENUPAGE_REGISTRY_VALUENAME "Start Menu Folder"

!define MUI_FINISHPAGE_RUN "$INSTDIR\sldshow2.exe"
!define MUI_FINISHPAGE_RUN_NOTCHECKED

Var StartMenuFolder
!insertmacro MUI_PAGE_STARTMENU Application $StartMenuFolder
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH

!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "Japanese"

Section
    SetOutPath "$INSTDIR"
    File "..\target\release\sldshow2.exe"
    File "..\README.md"
    File "..\LICENSE"
    File "..\example.sldshow"
    WriteUninstaller "$INSTDIR\uninstall.exe"

    # Computing EstimatedSize
    ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
    IntFmt $0 "0x%08X" $0

    WriteRegStr HKCU "${APP_REG_KEY}" "" "$INSTDIR"
    WriteRegStr HKCU "${APP_UNINST_KEY}" "DisplayIcon" "$INSTDIR\sldshow2.exe"
    WriteRegStr HKCU "${APP_UNINST_KEY}" "DisplayName" "${APP_NAME}"
    WriteRegStr HKCU "${APP_UNINST_KEY}" "DisplayVersion" "${APP_SEMANTIC_VERSION}"
    WriteRegStr HKCU "${APP_UNINST_KEY}" "Publisher" "ugai"
    WriteRegStr HKCU "${APP_UNINST_KEY}" "Readme" "$INSTDIR\README.md"
    WriteRegStr HKCU "${APP_UNINST_KEY}" "URLInfoAbout" "${APP_HOMEPAGE}"
    WriteRegStr HKCU "${APP_UNINST_KEY}" "UninstallString" "$INSTDIR\${APP_UNINST_EXE}"
    WriteRegStr HKCU "${APP_UNINST_KEY}" "QuietUninstallString" "$\"$INSTDIR\${APP_UNINST_EXE}$\" /S"
    WriteRegDWORD HKCU "${APP_UNINST_KEY}" "EstimatedSize" "$0"
    WriteRegDWORD HKCU "${APP_UNINST_KEY}" "NoModify" 1
    WriteRegDWORD HKCU "${APP_UNINST_KEY}" "NoRepair" 1

    !insertmacro MUI_STARTMENU_WRITE_BEGIN Application
        CreateShortcut "$SMPROGRAMS\sldshow2.lnk" "$INSTDIR\sldshow2.exe"
    !insertmacro MUI_STARTMENU_WRITE_END

    # File Association: .sldshow
    WriteRegStr HKCU "SOFTWARE\Classes\.sldshow" "" "sldshow2file"
    WriteRegStr HKCU "SOFTWARE\Classes\sldshow2file" "" "sldshow2"
    WriteRegStr HKCU "SOFTWARE\Classes\sldshow2file\DefaultIcon" "" "$INSTDIR\sldshow2.exe,0"
    WriteRegStr HKCU "SOFTWARE\Classes\sldshow2file\shell" "" "open"
    WriteRegStr HKCU "SOFTWARE\Classes\sldshow2file\shell\open\command" "" '"$INSTDIR\sldshow2.exe" "%1"'
SectionEnd

Section "Uninstall"
    Delete "$INSTDIR\uninstall.exe"
    Delete "$INSTDIR\sldshow2.exe"
    Delete "$INSTDIR\README.md"
    Delete "$INSTDIR\LICENSE"
    Delete "$INSTDIR\example.sldshow"
    RMDir "$INSTDIR"

    Delete "$SMPROGRAMS\sldshow2.lnk"

    DeleteRegKey HKCU "${APP_REG_KEY}"
    DeleteRegKey HKCU "${APP_UNINST_KEY}"

    # File Association
    DeleteRegKey HKCU "SOFTWARE\Classes\.sldshow"
    DeleteRegKey HKCU "SOFTWARE\Classes\sldshow2file"
SectionEnd

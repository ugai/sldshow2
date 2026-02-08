; sldshow2 Installer Script
; Requires NSIS to compile

!include "MUI2.nsh"

Name "sldshow2"
OutFile "sldshow2_setup.exe"
InstallDir "$LOCALAPPDATA\sldshow2"
InstallDirRegKey HKCU "Software\sldshow2" ""
RequestExecutionLevel user

;--------------------------------
; Interface Settings

!define MUI_ABORTWARNING
!define MUI_ICON "${NSISDIR}\Contrib\Graphics\Icons\modern-install.ico"
!define MUI_UNICODE true

;--------------------------------
; Pages

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

;--------------------------------
; Languages

!insertmacro MUI_LANGUAGE "English"

;--------------------------------
; Installer Sections

Section "sldshow2" SecDummy

  SetOutPath "$INSTDIR"
  
  ; ADD YOUR FILES HERE
  File "target\release\sldshow2.exe"
  File "LICENSE"
  File "README.md"
  
  ; Store installation folder
  WriteRegStr HKCU "Software\sldshow2" "" $INSTDIR
  
  ; Create uninstaller
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  
  ; Create Shortcuts
  CreateDirectory "$SMPROGRAMS\sldshow2"
  CreateShortcut "$SMPROGRAMS\sldshow2\sldshow2.lnk" "$INSTDIR\sldshow2.exe"
  CreateShortcut "$SMPROGRAMS\sldshow2\Uninstall.lnk" "$INSTDIR\Uninstall.exe"

SectionEnd

;--------------------------------
; Uninstaller Section

Section "Uninstall"

  Delete "$INSTDIR\sldshow2.exe"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\Uninstall.exe"

  RMDir "$INSTDIR"
  
  Delete "$SMPROGRAMS\sldshow2\sldshow2.lnk"
  Delete "$SMPROGRAMS\sldshow2\Uninstall.lnk"
  RMDir "$SMPROGRAMS\sldshow2"

  DeleteRegKey /ifempty HKCU "Software\sldshow2"

SectionEnd

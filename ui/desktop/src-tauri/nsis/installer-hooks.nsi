!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Locating OmniEdge Helper binary..."
  
  ; Ask the user if they want to install the background service
  MessageBox MB_YESNO|MB_ICONQUESTION "Would you like to install the OmniEdge Helper Service?$\n$\nThis service allows OmniEdge to connect to the VPN in the background and without requiring Administrator privileges every time you start the app." IDNO skip_helper_install

  ; Try the sidecar naming convention in subfolder first (Tauri 2 standard)
  IfFileExists "$INSTDIR\omni-helper-x86_64-pc-windows-msvc.exe" 0 +3
    StrCpy $1 "$INSTDIR\omni-helper-x86_64-pc-windows-msvc.exe"
    Goto done_finding
    
  ; Try the unversioned name in the root
  IfFileExists "$INSTDIR\omni-helper.exe" 0 +3
    StrCpy $1 "$INSTDIR\omni-helper.exe"
    Goto done_finding

  ; If still not found, check the binaries/ subfolder
  IfFileExists "$INSTDIR\binaries\omni-helper-x86_64-pc-windows-msvc.exe" 0 +3
    StrCpy $1 "$INSTDIR\binaries\omni-helper-x86_64-pc-windows-msvc.exe"
    Goto done_finding

  DetailPrint "WARNING: OmniEdge Helper binary not found! Service will not be installed."
  Goto finish

done_finding:
  DetailPrint "Updating OmniEdge Helper Service using binary: $1"
  ; Stop and delete existing service to ensure a clean install
  ExecWait 'cmd.exe /c sc.exe stop OmniEdgeHelper'
  ExecWait 'cmd.exe /c sc.exe delete OmniEdgeHelper'
  
  ; Use PowerShell to create the service for better error handling and UTF8 support if needed
  ExecWait "powershell.exe -NoProfile -Command $\"& { New-Service -Name OmniEdgeHelper -BinaryPathName ([char]34 + '$1' + [char]34) -DisplayName 'OmniEdge Helper Service' -StartupType Automatic }$\""
  ExecWait 'cmd.exe /c sc.exe description OmniEdgeHelper "Facilitates secure P2P networking for OmniEdge."'
  ExecWait 'cmd.exe /c sc.exe start OmniEdgeHelper'
  Goto finish

skip_helper_install:
  DetailPrint "User declined OmniEdge Helper Service installation."

finish:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Stopping OmniEdge Helper Service..."
  ExecWait 'cmd.exe /c sc.exe stop OmniEdgeHelper'
  DetailPrint "Removing OmniEdge Helper Service..."
  ExecWait 'cmd.exe /c sc.exe delete OmniEdgeHelper'
!macroend

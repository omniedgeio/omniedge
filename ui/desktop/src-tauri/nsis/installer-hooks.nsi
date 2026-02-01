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
  DetailPrint "Installing OmniEdge Helper Service using binary: $1"
  
  ; Step 1: Check if service exists and stop it
  DetailPrint "Checking for existing service..."
  nsExec::ExecToLog 'sc.exe query OmniEdgeHelper'
  Pop $0
  ${If} $0 == 0
    DetailPrint "Stopping existing service..."
    nsExec::ExecToLog 'sc.exe stop OmniEdgeHelper'
    ; Wait for service to stop
    Sleep 2000
    
    ; Force kill if still running
    nsExec::ExecToLog 'taskkill /F /IM omni-helper.exe'
    Sleep 1000
    
    DetailPrint "Removing existing service..."
    nsExec::ExecToLog 'sc.exe delete OmniEdgeHelper'
    ; Wait for deletion to complete
    Sleep 2000
  ${EndIf}
  
  ; Step 2: Create the service using sc.exe (more reliable than PowerShell New-Service)
  DetailPrint "Creating OmniEdge Helper Service..."
  nsExec::ExecToLog 'sc.exe create OmniEdgeHelper binPath= "$1" DisplayName= "OmniEdge Helper Service" start= auto'
  Pop $0
  ${If} $0 != 0
    DetailPrint "Warning: sc.exe create returned $0, trying alternative method..."
    ; Try PowerShell as fallback
    nsExec::ExecToLog 'powershell.exe -NoProfile -Command "New-Service -Name OmniEdgeHelper -BinaryPathName ([char]34 + ''$1'' + [char]34) -DisplayName ''OmniEdge Helper Service'' -StartupType Automatic -ErrorAction SilentlyContinue"'
  ${EndIf}
  
  ; Step 3: Set service description
  DetailPrint "Setting service description..."
  nsExec::ExecToLog 'sc.exe description OmniEdgeHelper "Facilitates secure P2P networking for OmniEdge VPN connections."'
  
  ; Step 4: Configure recovery options (restart on failure)
  DetailPrint "Configuring service recovery options..."
  nsExec::ExecToLog 'sc.exe failure OmniEdgeHelper reset= 86400 actions= restart/5000/restart/10000/restart/30000'
  
  ; Step 5: Start the service
  DetailPrint "Starting OmniEdge Helper Service..."
  nsExec::ExecToLog 'sc.exe start OmniEdgeHelper'
  Pop $0
  ${If} $0 != 0
    DetailPrint "Warning: Service start returned $0. The service may need a system restart."
  ${Else}
    DetailPrint "OmniEdge Helper Service started successfully!"
  ${EndIf}
  
  Goto finish

skip_helper_install:
  DetailPrint "User declined OmniEdge Helper Service installation."

finish:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Stopping OmniEdge Helper Service..."
  
  ; Stop the service gracefully
  nsExec::ExecToLog 'sc.exe stop OmniEdgeHelper'
  Sleep 2000
  
  ; Force kill if still running
  nsExec::ExecToLog 'taskkill /F /IM omni-helper.exe'
  Sleep 1000
  
  DetailPrint "Removing OmniEdge Helper Service..."
  nsExec::ExecToLog 'sc.exe delete OmniEdgeHelper'
  
  ; Wait for deletion
  Sleep 1000
  
  DetailPrint "OmniEdge Helper Service removed."
!macroend

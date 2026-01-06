import os

REPOS_PATH = "/Users/yong/Data/Project-OmniEdge/Git/omniedge-cli"
CMD_PATH = os.path.join(REPOS_PATH, "cmd/edgecli/cmd")
MAIN_PATH = os.path.join(REPOS_PATH, "cmd/edgecli/main.go")

API_MEMBERS = [
    "AuthResp", "AuthMethod", "LoginBySecretKey", "LoginByPassword", "AuthOption",
    "RefreshTokenOption", "AuthService", "HttpOption", "HandleCall",
    "DeviceResponse", "RegisterOption", "RegisterService", "HeartbeatOption",
    "HeartbeatService", "VirtualNetworkResponse", "VirtualNetworkService",
    "JoinOption", "JoinVirtualNetworkResponse", "SuccessResponse", "ErrorResponse"
]

CORE_MEMBERS = [
    "LoadClientConfig", "ConfigV", "HandleFilePrefix", "HandleFileStatus",
    "RevealHardwareUUID", "RevealHostName", "RevealOS", "GenerateRandomMac",
    "GetCurrentDeviceNetStatus", "ParseCIDR", "Edge", "StartOption", "StartService",
    "Env"
]

def refactor_file(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    # Replace import
    old_import = 'edge "github.com/omniedgeio/omniedge-cli"'
    new_imports = 'api "github.com/omniedgeio/omniedge-cli/pkg/api"\n\tcore "github.com/omniedgeio/omniedge-cli/pkg/core"'
    
    if old_import in content:
        content = content.replace(old_import, new_imports)
    elif 'edgecli "github.com/omniedgeio/omniedge-cli"' in content:
        content = content.replace('edgecli "github.com/omniedgeio/omniedge-cli"', 'core "github.com/omniedgeio/omniedge-cli/pkg/core"')
        content = content.replace('edgecli.', 'core.')

    # Replace members
    for member in API_MEMBERS:
        content = content.replace(f"edge.{member}", f"api.{member}")
    
    for member in CORE_MEMBERS:
        content = content.replace(f"edge.{member}", f"core.{member}")

    with open(filepath, 'w') as f:
        f.write(content)

# Refactor cmd files
for filename in os.listdir(CMD_PATH):
    if filename.endswith(".go"):
        refactor_file(os.path.join(CMD_PATH, filename))

# Refactor main.go
refactor_file(MAIN_PATH)

print("Refactoring complete.")

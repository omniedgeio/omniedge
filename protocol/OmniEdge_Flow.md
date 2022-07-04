Short description of the term.
- Proxy here I did not distinguish between Proxy, Worker, Web Backend
- Web here refers to the user's management interface, including and not limited to Web / App


## 1: User initialization/management network

```mermaid
sequenceDiagram
    participant O as OAuth2.0,Ldap
    participant U as User
    participant W as Web

    participant P as Proxy/Backend
    participant S as SNode

    rect rgb(0, 255, 120)
    %% auth2.0 block
        Note over O,W : Authentication Login
        U ->> W : Registered Login
        W ->> O : Redirect third party authentication
        alt User Confirmation
            U ->> O : User confirmation
            O ->> W : Complete authentication, return Token
        else User rejects
            U ->> O : User rejects or timeout
            O ->> W : Authentication failed, return error
        end
    end

    %% init network
    rect rgb(0, 255, 200)
        Note over U,S : Authentication completed to start network configuration
        U ->> W : Initialize VPN parameters (ID, IP, Gateway, route)
        W ->> P : Save network information
        P ->> S : Set VPN network information
        W ->> U : Return the network ID
        U ->> W : Create secret key pair
        W ->> P : Store the private and public keys
        P ->> S : Set Super Node private key
        Note over S : SuperNode Ready
        W ->> U : Return the public key

    end
```


## 2: Node joins the network for the first time


## Node joins the network


```mermaid
sequenceDiagram
    participant W as Web
    participant U as User
    participant N as Node

    participant P as Proxy/Backend
    participant S as SNode


    rect rgb(230, 150, 200)
    %% auth2.0 block
        Note over U, N: Node start
        alt configured file
            N ->> N : Read configuration file
        else no config file
            N -> U : Waiting for network parameters
            U -> N : User enters VPN ID and alias
        end
    end

    rect rgb(200, 200, 200)
    %% auth2.0 block
        Note over W,P: Node network information is obtained, explicitly via http or https
        N ->> P : Use device information, try to get device status and public key (ID, HW UUID, SW Ver)
        alt The current node is an unaudited device
            rect rgb(250, 250, 0)
                P ->> W: Display the device to be audited
                U ->> W: user authentication device
                W ->> P: mark the device as authenticated
                P --> N: return to normal, node continues
            end
        else blacklisted devices
            rect rgb(100, 100, 150)
            P ->> N: Notify node of exception, node goes offline
            N ->> N: node exits
            end
        else authenticated device
            rect rgb(0, 250, 150)
                P --> N: return normal, node continues
            end
        end
    end
    P ->> N: return VPN public key
    rect rgb(230, 150, 200)
        Note over W,S: Follow up with public key encryption communication (which actually affects performance)
        N ->> P: Request node network information
        P ->> N: assign P2P secret key and IP information
        N ->> P: request a list of information about all nodes in the network
        P ->> N: Return a list of information about all nodes in the network

        N ->> S: Join SuperNode network via Proxy
        S ->> P: Check if it is an authenticated device
        P ->> S: Verify that the device is legitimate
        S ->> N: Confirm join success

    end
    par heartbeat information
        rect rgb(230, 150, 200)
            loop Loop node and Super Node heartbeat signal
                N -->> S: send a heartbeat signal using the secret key assigned to the node
                S -->> P: Update the Proxy maintenance list
                S ->> N: reply to the heartbeat signal using the node's assigned secret key
            end
        end
    and normal communication
        rect rgb(230, 200, 150)
            loop Loop Node and Super Node communicate legally
                N --> S: Encrypted communication using the secret key and algorithm assigned by the node
                S -->> N: Active or passive reply to the node using the node's assigned secret key
            end
        end
    end
```

## 3: Node P2P communication

When communicating between two nodes, both parties need to make sure their P2P Profile List is up-to-date and each maintains information about the P2P devices in the network

### P2P direct penetration communication

```mermaid
sequenceDiagram

    participant N0 as legal node 0
    participant N1 as Legal Node 1

    participant P as Proxy
    participant S as SuperNode

    par heartbeat information
        rect rgb(230, 150, 200)
            loop Node update P2P Profile List
                N0 ->> P: request to update P2P Profile List
                P ->> N0: return the latest
            end
        end
    and normal communication
        rect rgb(230, 200, 150)
            loop Node update P2P Profile List
                N1 ->> P: request to update the P2P Profile List
                P ->> N1: return the latest
            end
        end
    end

    alt P2P direct connection
        rect rgb(255, 255, 0)
            N0 ->> N1 : Establish a communication link using the pass-through parameter
            rect rgb(0, 250, 150)
                loop Start formal communication (encryption optional))
                    N0 ->> N1 : hello (encrypt the payload using its own encryption method and secret key)
                    N1 ->> N0 : decrypt with the initiator's secret key, and reply with your own secret key if successful
                end
            end
        end
    else server relay
        rect rgb(255, 255, 0)
            N0 ->> S : Node and SuperNode establish a link, multiplex the heartbeat link
            N1 ->> S : Node and SuperNode establish a link, multiplex the heartbeat link
            rect rgb(0, 250, 150)
                loop Start formal communication (encryption optional))
                    N0 ->> S : hello (encrypt the payload using its own encryption method and secret key)
                    S ->> N1 : SuperNode verifies the node and then forwards to the target node
                    N1 ->> S : The receiver decrypts using the sender's secret key, and then replies with its own secret key encryption
                    S ->> N0 : SuperNode verifies the node and then forwards to the target node
                end
            end
        end
    end
```

## Security protection

### 1: Self-protection of nodes

Nodes may have possible data leakage during use


| Possibility | Description | Solution Ideas |
| ---- | ---- | ---- |
| public key data | public key is not worried about losing | try not to store it locally, and use https encryption during the acquisition process |
| https hijacking | https hijacking | calibrate https public key |
| P2P Profile List secret key leak | hole-punching and encryption information leakage throughout the network | data is updated regularly, node self-checking, proximity node detection and timely blackout |

### 2: Active prevention of data leakage

The possibility of leakage includes.

| possibility | description | solution ideas |
| ---- | ---- | ---- | 
| Storage | Store sensitive data locally | Try to store in memory and obfuscate |
| debug crack | memory intrusion crack | program self-check and add anti-debug code, and promptly notify the proxy to pull the hack |



## Brief description of data structure

P2P Profile List Info

For each device in the P2P network, there is a structure to describe its P2P profile as follows
| Key | Type | Comment |
| ---- | ---- | ---- |
| old n2n | struct | original n2n parameters |
| encryp_type | int | Encryption method, such as cha20 symmetric encryption |
| compress_type | int | Compression methods, such as gzip
| key | char[64] | The secret key of the current node |
| timeout | uint64_t | The time out deadline of the current profile |




## Data traffic analysis

```mermaid
pie title node traffic
    "P2P direct connection" : 60
    "Server transit" : 40
```
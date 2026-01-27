# On the exit node side

Add the -r to the join command: sudo omniedge join -r at the device(Cloud Instance) you want to set as an EXIT NODE
Enable packet forwarding: sudo sysctl -w net.ipv4.ip_forward=1
Enable IP masquerading: sudo iptables -t nat -A POSTROUTING -j MASQUERADE

# On the client side

Linux (works)

Prepare
EXIT_NODE_IP="100.100.100.1"
CUSTOMIZE_SUPERNODE_IP="11.22.33.44"
DNS_SERVER="8.8.8.8"
CURRENT_GW=$(ip route get 8.8.8.8 | head -n1 | awk '{ print $3 }')
SET
cp /etc/resolv.conf /etc/resolv.conf.my_bak
echo "nameserver $DNS_SERVER" > /etc/resolv.conf
ip route add $CUSTOMIZE_SUPERNODE_IP via "$CURRENT_GW"
ip route del default
ip route add default via $EXIT_NODE_IP
Restore
ip route del default
ip route del $CUSTOMIZE_SUPERNODE_IP via "$CURRENT_GW"
ip route add default via "$CURRENT_GW"
mv /etc/resolv.conf.my_bak /etc/resolv.conf


Windows (Waiting for test)

#Prepare 

EXIT_NODE_IP="100.100.100.1" #Get from api
CUSTOMIZE_SUPERNODE_IP="11.22.33.44" #Get from api
DNS_SERVER="8.8.8.8" #Get from api
CURRENT_GW=$(ip route get 8.8.8.8 | head -n1 | awk '{ print $3 }')

#Set
route delete
route ADD $CUSTOMIZE_SUPERNODE_IP MASK 255.255.255.0 $CURRENT_GW
route ADD 0.0.0.0 MASK 255.255.255.0 $EXIT_NODE_IP

#Restore
route delete $CUSTOMIZE_SUPERNODE_IP
route delete 0.0.0.0
route ADD 0.0.0.0 MASK 255.255.255.0 $CURRENT_GW
macOS (Waiting for test)

#Prepare 

EXIT_NODE_IP="100.100.100.1" #Get from api
CUSTOMIZE_SUPERNODE_IP="11.22.33.44" #Get from api
DNS_SERVER="8.8.8.8" #Get from api
CURRENT_GW=$(ip route get 8.8.8.8 | head -n1 | awk '{ print $3 }')

# Set
route -n add -net $CUSTOMIZE_SUPERNODE_IP $CURRENT_GW
route -n add -net 0.0.0.0 $EXIT_NODE_IP

# Restore
route delete -net $CUSTOMIZE_SUPERNODE_IP
route delete -net 0.0.0.0
route -n add -net 0.0.0.0 $CURRENT_GW
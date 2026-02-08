# lan-network

**Best-effort discovery of the machine's default gateway and local network IPv4 address exclusively with UDP sockets.**

This utility attempts to determine the interface and IPv4 address of a machine
on the local network. It'll attempt to determine the default gateway and
return the corresponding network interface assignment, both when the network
is online and offline.

The LAN Network it attempts to pick is the one that the machine uses to connect
to the internet. Determining it is useful to pick the machine's IP address that
is generally used to connect to it from other devices on the network.

`lanNetwork()` makes three separate attempts to guess the local network:

1. Create a socket to a publicly routed IP, and return the assignment matching the socket's local address
2. Broadcast DHCP discovery packets on all routable network assignments and listen for replies
3. Highest priority assignment

`lanNetworkSync()` does the same synchronously by spawning a child process
and blocking until a result is determined. Using this method is generally
not recommended.

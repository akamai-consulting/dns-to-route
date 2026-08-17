# dns-to-route


This is expected to run from an LKE-E with NAT Gateway for VPCs.

```
DESIRED_ROUTE="www.example.com eth0 10.0.0.1"

kubectl apply -f - <<EOF
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: lke-route-installer
  namespace: kube-system
  labels:
    app: route-installer
spec:
  selector:
    matchLabels:
      app: route-installer
  template:
    metadata:
      labels:
        app: route-installer
    spec:
      priorityClassName: system-node-critical
      hostNetwork: true
      tolerations:
      - operator: "Exists"
      containers:
      - name: route-maintainer
        image: customcontainer:latest
        securityContext:
          privileged: true
        command: ["/bin/sh", "-c"]
        args:
          - |
            echo "Starting loop for $DESIRED_ROUTE..."
            while true; do
              dns-to-route $DESIRED_ROUTE
              sleep 30
            done
        resources:
          limits:
            cpu: 50m
            memory: 50Mi
EOF
```


Command syntax:
```
usage:
        dns-to-route <DNS record to resolve> <interface> <source>
```

Example:
```
#> dns-to-route www.example.com eth0 172.236.110.200
Checking status of address: 104.20.23.154
No matching result exists. Adding DNS result 104.20.23.154 to the table.
Route for 104.20.23.154 added.
Checking status of address: 172.66.147.243
No matching result exists. Adding DNS result 172.66.147.243 to the table.
Route for 172.66.147.243 added.```
```

We did a DNS request on www.example.com and we recieved two IPv4 addresses back. We are going to add a route for each IP to use the eth0 interface and source from 172.236.110.200

If one or all of the routes exist, we will skip the step of adding the route:
```
#> ip route del 172.66.147.243 dev eth0 proto babel src 172.236.110.200
#> dns-to-route www.example.com eth0 172.236.110.200
Checking status of address: 172.66.147.243
No matching result exists. Adding DNS result 172.66.147.243 to the table.
Route for 172.66.147.243 added.
Checking status of address: 104.20.23.154
Route exists for 104.20.23.154
```

If DNS no longer reports an IP, we will remove that route:

```
Checking status of address: 104.20.23.154
Route exists for 8.8.8.8, but no longer in DNS.
Route for 8.8.8.8 deleted.
Route exists for 104.20.23.154
Checking status of address: 172.66.147.243
Route exists for 172.66.147.243
```

Clone repo, cd into dns-to-route, then run:
```
cargo build --release
```
To compile.

package main

import "tailscale.com/net/netns"

func init() {
	netns.SetEnabled(false)
}

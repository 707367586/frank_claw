// scripts/init-config — minimal replacement for `picoclaw onboard`.
//
// `cmd/picoclaw onboard` cannot build from this vendored tree because its
// //go:embed workspace directive matches no files (see backend/PATCHES.md).
// This program calls pkg/config.DefaultConfig() and writes a default
// ~/.picoclaw/config.json with the Pico channel pre-enabled (so the launcher
// can serve /pico/ws). Idempotent: if config already exists, leaves it alone.
//
// Usage:
//
//	go run ./scripts/init-config/    (from inside backend/)
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"github.com/sipeed/picoclaw/pkg/config"
)

func main() {
	home, err := os.UserHomeDir()
	if err != nil {
		die(err)
	}
	dir := filepath.Join(home, ".picoclaw")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		die(err)
	}
	path := filepath.Join(dir, "config.json")

	if _, err := os.Stat(path); err == nil {
		fmt.Println("config exists at", path, "— leaving untouched")
		return
	}

	cfg := config.DefaultConfig()
	// Pre-enable the Pico channel so the launcher serves /pico/ws.
	// ChannelsConfig is map[string]*Channel, so we index by key "pico".
	if ch, ok := cfg.Channels["pico"]; ok {
		ch.Enabled = true
	}

	f, err := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if err != nil {
		die(err)
	}
	defer f.Close()
	enc := json.NewEncoder(f)
	enc.SetIndent("", "  ")
	if err := enc.Encode(cfg); err != nil {
		die(err)
	}
	fmt.Println("wrote", path)
}

func die(err error) {
	fmt.Fprintln(os.Stderr, "init-config:", err)
	os.Exit(1)
}

//go:build !darwin

package platform

import "context"

// FindFiles returns nothing: TinyDash uses no OS file index here, as
// desktops on Linux use different ones, and Windows Search is not wired.
func FindFiles(ctx context.Context, name, folder string, limit int) []string { return nil }

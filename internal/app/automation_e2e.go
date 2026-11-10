//go:build e2e

package app

// The e2e build listens on the socket that MYGO_AUTOMATION names, so the
// verify-tinydash skill can drive the real app. Release builds leave it out.
import _ "github.com/egoist/mygo/plugins/automation"

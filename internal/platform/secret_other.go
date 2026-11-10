//go:build !darwin

package platform

// WriteSecretText leaves the write to the app, whose clipboard writes the
// marker formats by name on Windows and Linux.
func WriteSecretText(string) (handled bool, err error) { return false, nil }

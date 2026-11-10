package store

import (
	"bytes"
	"encoding/json"
	"fmt"
	"strings"
	"unicode/utf8"

	"github.com/jewei/tinydash/internal/feature/clipboard"
)

// text and integer read a column as rusqlite's typed getters did: a value
// of another storage class is an error, not a conversion.
func text(v any) (string, error) {
	s, ok := v.(string)
	if !ok {
		return "", fmt.Errorf("column holds %T, want text", v)
	}
	return s, nil
}

func integer(v any) (int64, error) {
	n, ok := v.(int64)
	if !ok {
		return 0, fmt.Errorf("column holds %T, want an integer", v)
	}
	return n, nil
}

// optionalUint32 reads a width or height; NULL is 0.
func optionalUint32(v any) (uint32, error) {
	if v == nil {
		return 0, nil
	}
	n, err := integer(v)
	if err == nil && (n < 0 || n > int64(^uint32(0))) {
		err = fmt.Errorf("%d is out of range", n)
	}
	return uint32(n), err
}

func clipKind(v any) (clipboard.Kind, error) {
	s, err := text(v)
	if err != nil {
		return "", err
	}
	switch kind := clipboard.Kind(s); kind {
	case clipboard.KindText, clipboard.KindImage, clipboard.KindFiles:
		return kind, nil
	}
	return "", fmt.Errorf("clipboard entry of unknown kind %q", s)
}

// encodePaths writes a file list as the Rust app did: a compact JSON array
// without HTML escapes. A path that is not UTF-8 fails, as it did there.
func encodePaths(paths []string) (string, error) {
	var out bytes.Buffer
	encoder := json.NewEncoder(&out)
	encoder.SetEscapeHTML(false)
	for _, path := range paths {
		if !utf8.ValidString(path) {
			return "", fmt.Errorf("path %q is not UTF-8", path)
		}
	}
	if err := encoder.Encode(paths); err != nil {
		return "", err
	}
	return strings.TrimSuffix(out.String(), "\n"), nil
}

// decodePaths reads a stored file list. It must be an array of strings:
// serde refused null and other types, which Go's decoder would let through.
func decodePaths(content string) ([]string, error) {
	var values []any
	if err := json.Unmarshal([]byte(content), &values); err != nil {
		return nil, err
	}
	if values == nil {
		return nil, fmt.Errorf("file list is %s, want an array", content)
	}
	paths := make([]string, len(values))
	for i, v := range values {
		path, ok := v.(string)
		if !ok {
			return nil, fmt.Errorf("file list holds %v, want paths", v)
		}
		paths[i] = path
	}
	return paths, nil
}

// Package zoneinfo loads time zone rules from a copy of the IANA database
// that is built into the binary. time.LoadLocation reads $ZONEINFO and the
// system's zoneinfo files before its own embedded copy, so its answers
// depend on the host, and a search could wait on the disk.
package zoneinfo

import (
	"archive/zip"
	"bytes"
	_ "embed"
	"fmt"
	"io"
	"sync"
	"time"
)

// archive is $(go env GOROOT)/lib/time/zoneinfo.zip of Go 1.27.3, IANA
// release 2026c: the same rules that time/tzdata embeds.
//
//go:embed zoneinfo.zip
var archive []byte

var files = sync.OnceValues(func() (map[string]*zip.File, error) {
	r, err := zip.NewReader(bytes.NewReader(archive), int64(len(archive)))
	if err != nil {
		return nil, fmt.Errorf("read the built-in time zones: %w", err)
	}
	out := make(map[string]*zip.File, len(r.File))
	for _, f := range r.File {
		out[f.Name] = f
	}
	return out, nil
})

// Load returns the zone with an IANA name, such as "Asia/Kuala_Lumpur".
func Load(name string) (*time.Location, error) {
	index, err := files()
	if err != nil {
		return nil, err
	}
	f, ok := index[name]
	if !ok {
		return nil, fmt.Errorf("unknown time zone %q", name)
	}
	rc, err := f.Open()
	if err != nil {
		return nil, err
	}
	defer rc.Close()
	data, err := io.ReadAll(rc)
	if err != nil {
		return nil, err
	}
	return time.LoadLocationFromTZData(name, data)
}

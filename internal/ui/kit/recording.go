package kit

// Recording is what a shortcut recorder shows: whether it records, and
// what it has to say about its last attempt, a recording or a save. The
// launcher's hotkey dialog and the Settings shortcut both use it, so they
// keep one set of rules:
//   - Start and Save begin an attempt with nothing to say, and return its
//     number. Each attempt shows its own result, never one left from the
//     attempt before it.
//   - Fail gives the attempt's own reason, a refused pause or save. It
//     replaces what was there, and recording stops.
//   - Resumed takes the outcome of turning the shortcuts on again. A
//     refusal there is about other keys, so it shows only when the
//     attempt has nothing else to say.
//   - Fail and Resumed take the attempt's number: a reply that comes after
//     a newer attempt began is about the old one, so it is dropped.
//
// The zero value records nothing and has nothing to say.
type Recording struct {
	on  bool
	err string
	n   int
}

// On is whether the recorder records.
func (r *Recording) On() bool { return r.on }

// Err is what the recorder has to say, or "".
func (r *Recording) Err() string { return r.err }

// Last is the number of the last attempt.
func (r *Recording) Last() int { return r.n }

// Start begins recording.
func (r *Recording) Start() int {
	r.n++
	r.on, r.err = true, ""
	return r.n
}

// Save begins a save that did not record, such as removing the keys.
func (r *Recording) Save() int {
	r.n++
	r.on, r.err = false, ""
	return r.n
}

// Stop ends the recording and keeps what it has to say.
func (r *Recording) Stop() { r.on = false }

// Fail stops attempt n with its own reason, if err is set.
func (r *Recording) Fail(n int, err error) {
	if err != nil && n == r.n {
		r.on, r.err = false, err.Error()
	}
}

// Resumed takes a resume's outcome after attempt n: its refusal shows only
// when the attempt has nothing else to say.
func (r *Recording) Resumed(n int, err error) {
	if err != nil && n == r.n && r.err == "" {
		r.err = err.Error()
	}
}

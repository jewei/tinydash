package core

// Use is how often and how lately a result ran, for ranking.
type Use struct {
	Count    uint32
	LastUsed int64 // Unix seconds
}

// Personal is what ranking knows about the user: the usage bonus and pins.
// Sources take it in their search context; nil means no usage and no pins.
type Personal interface {
	// Boost is the usage bonus of a result.
	Boost(id ID) uint32
	Pinned(id ID) bool
}

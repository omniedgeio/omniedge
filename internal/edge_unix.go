//go:build !windows
// +build !windows

package omnin2n

import (
	"github.com/omniedgeio/omniedge/internal/coren2n"
)

// Edge is a type alias to the real implementation in the coren2n sub-package.
// This allows Unix systems to use the CGO-dependent implementation while
// isolating it from the main Go tool scanning during Windows builds.
type Edge = coren2n.Edge

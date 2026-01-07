package main

import (
	"embed"

	"github.com/omniedgeio/omniedge-cli/pkg/bridge"
	"github.com/wailsapp/wails/v2"
	"github.com/wailsapp/wails/v2/pkg/options"
	"github.com/wailsapp/wails/v2/pkg/options/assetserver"
	"github.com/wailsapp/wails/v2/pkg/options/mac"
)

//go:embed all:frontend/dist
var assets embed.FS

func main() {
	// Create the OmniEdge Bridge
	edgeBridge := bridge.NewBridge()

	// Create application with options
	err := wails.Run(&options.App{
		Title:     "OmniEdge",
		Width:     420,
		Height:    680,
		MinWidth:  380,
		MinHeight: 600,
		AssetServer: &assetserver.Options{
			Assets: assets,
		},
		BackgroundColour: &options.RGBA{R: 15, G: 17, B: 28, A: 255},
		OnStartup:        edgeBridge.SetContext,
		Bind: []interface{}{
			edgeBridge,
		},
		Mac: &mac.Options{
			TitleBar:             mac.TitleBarHiddenInset(),
			Appearance:           mac.NSAppearanceNameDarkAqua,
			WebviewIsTransparent: true,
			WindowIsTranslucent:  true,
		},
	})

	if err != nil {
		println("Error:", err.Error())
	}
}

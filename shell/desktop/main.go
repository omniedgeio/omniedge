package main

import (
	"embed"
	_ "embed"
	"log"

	"github.com/wailsapp/wails/v3/pkg/application"
)

//go:embed all:frontend/dist
var assets embed.FS

//go:embed build/appicon.png
var appIcon []byte

//go:embed build/trayicon_template.png
var trayIconTemplate []byte

//go:embed build/trayicon_connected.png
var trayIconConnected []byte

//go:embed build/trayicon_disconnected.png
var trayIconDisconnected []byte

func main() {
	// Create the OmniEdge Bridge Service
	bridgeService := NewBridgeService()

	// Create a new Wails application
	app := application.New(application.Options{
		Name:        "OmniEdge",
		Description: "OmniEdge VPN Client",
		Services: []application.Service{
			application.NewService(bridgeService),
		},
		Assets: application.AssetOptions{
			Handler: application.AssetFileServerFS(assets),
		},
		Mac: application.MacOptions{
			ApplicationShouldTerminateAfterLastWindowClosed: false,
		},
	})

	// Set the app reference in bridge service
	bridgeService.SetApp(app)
	bridgeService.SetAppIcon(appIcon)

	// Create the main window (hidden by default)
	mainWindow := app.Window.NewWithOptions(application.WebviewWindowOptions{
		Title:     "OmniEdge",
		Width:     320,
		Height:    600,  // Initial height, resized by frontend
		MinWidth:  320,  // Lock width
		MaxWidth:  320,  // Lock width
		MinHeight: 600,  // Allow shrinking
		MaxHeight: 1000, // Allow growing
		Mac: application.MacWindow{
			InvisibleTitleBarHeight: 50,
			Backdrop:                application.MacBackdropTranslucent,
			TitleBar:                application.MacTitleBarHiddenInset,
		},
		BackgroundColour: application.NewRGBA(0, 0, 0, 0),
		URL:              "/",
		Hidden:           true,
		AlwaysOnTop:      true,
		Frameless:        true,
	})

	// Create system tray menu
	trayMenu := app.Menu.New()
	trayMenu.Add("Show OmniEdge").OnClick(func(*application.Context) {
		mainWindow.Show()
		mainWindow.Focus()
	})
	trayMenu.AddSeparator()
	trayMenu.Add("Quit").OnClick(func(*application.Context) {
		// Disconnect before quitting
		bridgeService.Disconnect()
		app.Quit()
	})

	// Note: Quit handled via BridgeService.Quit() method called from frontend

	// Create system tray
	systemTray := app.SystemTray.New()
	systemTray.SetIcon(trayIconTemplate) // Start with template/disconnected icon
	systemTray.SetTooltip("OmniEdge")
	systemTray.SetMenu(trayMenu)

	// Set the tray in bridge service for dynamic icon switching
	bridgeService.SetSystemTray(systemTray, trayIconConnected, trayIconDisconnected)

	// Set main window reference for dynamic resizing
	bridgeService.SetMainWindow(mainWindow)

	// Attach window to tray
	// This handles the show/hide on tray icon click automatically in Wails v3
	systemTray.AttachWindow(mainWindow).WindowOffset(5)

	// Run the application
	err := app.Run()
	if err != nil {
		log.Fatal(err)
	}
}

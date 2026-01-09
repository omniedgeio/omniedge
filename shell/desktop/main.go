package main

import (
	"embed"
	_ "embed"
	"log"
	"runtime"

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
		Description: "OmniEdge Client",
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

	// Create the main window with platform-specific options
	windowOpts := application.WebviewWindowOptions{
		Title:     "OmniEdge",
		Width:     320,
		Height:    480, // Reduced initial height for a more compact popover
		MinWidth:  320,
		MaxWidth:  320,
		MinHeight: 200,
		MaxHeight: 800,
		URL:       "/",
		Hidden:    true,
		Frameless: true, // Always frameless for the popover effect
	}

	// Platform-specific window styling
	switch runtime.GOOS {
	case "darwin":
		windowOpts.Mac = application.MacWindow{
			InvisibleTitleBarHeight: 0, // No title bar needed for popover
			Backdrop:                application.MacBackdropTranslucent,
			TitleBar:                application.MacTitleBarHidden,
		}
		windowOpts.BackgroundColour = application.NewRGBA(0, 0, 0, 0)
		windowOpts.AlwaysOnTop = true
	case "windows":
		// Windows: Use system chrome with transparency if available
		windowOpts.BackgroundColour = application.NewRGBA(255, 255, 255, 255)
		windowOpts.AlwaysOnTop = true
		windowOpts.Frameless = true // Also frameless on Windows for consistency
	case "linux":
		// Linux: Standard window with native chrome
		windowOpts.BackgroundColour = application.NewRGBA(255, 255, 255, 255)
		windowOpts.AlwaysOnTop = true
		windowOpts.Frameless = true // Also frameless on Linux for consistency
	}

	mainWindow := app.Window.NewWithOptions(windowOpts)

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

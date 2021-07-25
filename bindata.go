// Code generated for package edgecli by go-bindata DO NOT EDIT. (@generated)
// sources:
// config/dev.yml
// config/prod.yml
package edgecli

import (
	"bytes"
	"compress/gzip"
	"fmt"
	"io"
	"io/ioutil"
	"os"
	"path/filepath"
	"strings"
	"time"
)

func bindataRead(data []byte, name string) ([]byte, error) {
	gz, err := gzip.NewReader(bytes.NewBuffer(data))
	if err != nil {
		return nil, fmt.Errorf("Read %q: %v", name, err)
	}

	var buf bytes.Buffer
	_, err = io.Copy(&buf, gz)
	clErr := gz.Close()

	if err != nil {
		return nil, fmt.Errorf("Read %q: %v", name, err)
	}
	if clErr != nil {
		return nil, err
	}

	return buf.Bytes(), nil
}

type asset struct {
	bytes []byte
	info  os.FileInfo
}

type bindataFileInfo struct {
	name    string
	size    int64
	mode    os.FileMode
	modTime time.Time
}

// Name return file name
func (fi bindataFileInfo) Name() string {
	return fi.name
}

// Size return file size
func (fi bindataFileInfo) Size() int64 {
	return fi.size
}

// Mode return file mode
func (fi bindataFileInfo) Mode() os.FileMode {
	return fi.mode
}

// Mode return file modify time
func (fi bindataFileInfo) ModTime() time.Time {
	return fi.modTime
}

// IsDir return file whether a directory
func (fi bindataFileInfo) IsDir() bool {
	return fi.mode&os.ModeDir != 0
}

// Sys return file is sys mode
func (fi bindataFileInfo) Sys() interface{} {
	return nil
}

var _configDevYml = []byte("\x1f\x8b\x08\x00\x00\x00\x00\x00\x00\xff\x7c\xce\x31\x8e\x83\x30\x10\x05\xd0\x7e\x4f\xb1\x17\xb0\xbd\x4b\x04\x91\xa8\xd3\xa4\xc9\x1d\x06\x33\x60\x83\x19\x1b\x7b\x0c\x09\x51\xee\x1e\x45\x41\x4a\x97\xf2\xff\xe2\xbf\xaf\x9d\x45\x62\x31\x78\x4b\x22\x47\x57\xff\x1a\xe6\x90\x6a\xa5\x0e\xcd\x9f\xf3\xa6\xf8\x4f\x8d\xc4\x2b\xea\xcc\x28\x20\x58\x99\x93\x40\x48\x2c\x0a\x09\x13\x6c\x9e\x60\x4d\x52\xfb\x49\xb5\xb8\xa8\xc5\x46\xce\xe0\x04\x21\xaf\x3e\x8e\xea\xbe\x17\x97\x77\x3e\x9f\x1e\xea\xe5\xfc\xf4\x11\x82\x99\x9d\x40\x6a\x83\xb7\xc4\x1f\x94\x4c\xcf\x65\xe0\xa6\xec\xcc\xb0\xf5\xb9\x9a\xa7\xb5\x3b\x56\xc6\x8c\x20\x21\x84\x74\x23\xfd\xf5\xc4\x3e\xfc\x0c\x00\x00\xff\xff\x41\x60\xc6\x3a\xd4\x00\x00\x00")

func configDevYmlBytes() ([]byte, error) {
	return bindataRead(
		_configDevYml,
		"config/dev.yml",
	)
}

func configDevYml() (*asset, error) {
	bytes, err := configDevYmlBytes()
	if err != nil {
		return nil, err
	}

	info := bindataFileInfo{name: "config/dev.yml", size: 212, mode: os.FileMode(420), modTime: time.Unix(1616857360, 0)}
	a := &asset{bytes: bytes, info: info}
	return a, nil
}

var _configProdYml = []byte("\x1f\x8b\x08\x00\x00\x00\x00\x00\x00\xff\x01\x00\x00\xff\xff\x00\x00\x00\x00\x00\x00\x00\x00")

func configProdYmlBytes() ([]byte, error) {
	return bindataRead(
		_configProdYml,
		"config/prod.yml",
	)
}

func configProdYml() (*asset, error) {
	bytes, err := configProdYmlBytes()
	if err != nil {
		return nil, err
	}

	info := bindataFileInfo{name: "config/prod.yml", size: 0, mode: os.FileMode(420), modTime: time.Unix(1616685625, 0)}
	a := &asset{bytes: bytes, info: info}
	return a, nil
}

// Asset loads and returns the asset for the given name.
// It returns an error if the asset could not be found or
// could not be loaded.
func Asset(name string) ([]byte, error) {
	cannonicalName := strings.Replace(name, "\\", "/", -1)
	if f, ok := _bindata[cannonicalName]; ok {
		a, err := f()
		if err != nil {
			return nil, fmt.Errorf("Asset %s can't read by error: %v", name, err)
		}
		return a.bytes, nil
	}
	return nil, fmt.Errorf("Asset %s not found", name)
}

// MustAsset is like Asset but panics when Asset would return an error.
// It simplifies safe initialization of global variables.
func MustAsset(name string) []byte {
	a, err := Asset(name)
	if err != nil {
		panic("asset: Asset(" + name + "): " + err.Error())
	}

	return a
}

// AssetInfo loads and returns the asset info for the given name.
// It returns an error if the asset could not be found or
// could not be loaded.
func AssetInfo(name string) (os.FileInfo, error) {
	cannonicalName := strings.Replace(name, "\\", "/", -1)
	if f, ok := _bindata[cannonicalName]; ok {
		a, err := f()
		if err != nil {
			return nil, fmt.Errorf("AssetInfo %s can't read by error: %v", name, err)
		}
		return a.info, nil
	}
	return nil, fmt.Errorf("AssetInfo %s not found", name)
}

// AssetNames returns the names of the assets.
func AssetNames() []string {
	names := make([]string, 0, len(_bindata))
	for name := range _bindata {
		names = append(names, name)
	}
	return names
}

// _bindata is a table, holding each asset generator, mapped to its name.
var _bindata = map[string]func() (*asset, error){
	"config/dev.yml":  configDevYml,
	"config/prod.yml": configProdYml,
}

// AssetDir returns the file names below a certain
// directory embedded in the file by go-bindata.
// For example if you run go-bindata on data/... and data contains the
// following hierarchy:
//     data/
//       foo.txt
//       img/
//         a.png
//         b.png
// then AssetDir("data") would return []string{"foo.txt", "img"}
// AssetDir("data/img") would return []string{"a.png", "b.png"}
// AssetDir("foo.txt") and AssetDir("notexist") would return an error
// AssetDir("") will return []string{"data"}.
func AssetDir(name string) ([]string, error) {
	node := _bintree
	if len(name) != 0 {
		cannonicalName := strings.Replace(name, "\\", "/", -1)
		pathList := strings.Split(cannonicalName, "/")
		for _, p := range pathList {
			node = node.Children[p]
			if node == nil {
				return nil, fmt.Errorf("Asset %s not found", name)
			}
		}
	}
	if node.Func != nil {
		return nil, fmt.Errorf("Asset %s not found", name)
	}
	rv := make([]string, 0, len(node.Children))
	for childName := range node.Children {
		rv = append(rv, childName)
	}
	return rv, nil
}

type bintree struct {
	Func     func() (*asset, error)
	Children map[string]*bintree
}

var _bintree = &bintree{nil, map[string]*bintree{
	"config": &bintree{nil, map[string]*bintree{
		"dev.yml":  &bintree{configDevYml, map[string]*bintree{}},
		"prod.yml": &bintree{configProdYml, map[string]*bintree{}},
	}},
}}

// RestoreAsset restores an asset under the given directory
func RestoreAsset(dir, name string) error {
	data, err := Asset(name)
	if err != nil {
		return err
	}
	info, err := AssetInfo(name)
	if err != nil {
		return err
	}
	err = os.MkdirAll(_filePath(dir, filepath.Dir(name)), os.FileMode(0755))
	if err != nil {
		return err
	}
	err = ioutil.WriteFile(_filePath(dir, name), data, info.Mode())
	if err != nil {
		return err
	}
	err = os.Chtimes(_filePath(dir, name), info.ModTime(), info.ModTime())
	if err != nil {
		return err
	}
	return nil
}

// RestoreAssets restores an asset under the given directory recursively
func RestoreAssets(dir, name string) error {
	children, err := AssetDir(name)
	// File
	if err != nil {
		return RestoreAsset(dir, name)
	}
	// Dir
	for _, child := range children {
		err = RestoreAssets(dir, filepath.Join(name, child))
		if err != nil {
			return err
		}
	}
	return nil
}

func _filePath(dir, name string) string {
	cannonicalName := strings.Replace(name, "\\", "/", -1)
	return filepath.Join(append([]string{dir}, strings.Split(cannonicalName, "/")...)...)
}

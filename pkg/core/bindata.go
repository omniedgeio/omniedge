// Code generated for package core by go-bindata DO NOT EDIT. (@generated)
// sources:
// config/dev.yml
// config/prod.yml
package core

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

var _configDevYml = []byte("\x1f\x8b\x08\x00\x00\x00\x00\x00\x00\xff\x74\xcb\x4b\x0a\x03\x21\x0c\x00\xd0\x7d\x4f\x51\xe8\xda\x04\x13\xe8\x27\xb7\x11\x0c\x6d\xa0\xa3\x41\x33\x9e\x7f\x56\xb3\x9c\x03\xbc\xa1\x33\x92\xb6\xea\xdd\x5a\xa4\x7d\xfc\xe5\xfe\x8b\xf0\x29\x88\x55\x57\x2a\x6e\xd0\xb7\x66\x5a\xbf\x0a\xd6\xb1\xb8\xe1\xa2\xdb\xe3\x82\x09\x62\xfe\x10\xe4\xe7\x1b\x98\x80\xe8\x25\xcc\xcc\xa7\x3a\x02\x00\x00\xff\xff\x36\xc0\x7d\x43\x6c\x00\x00\x00")

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

	info := bindataFileInfo{name: "config/dev.yml", size: 108, mode: os.FileMode(420), modTime: time.Unix(1650282249, 0)}
	a := &asset{bytes: bytes, info: info}
	return a, nil
}

var _configProdYml = []byte("\x1f\x8b\x08\x00\x00\x00\x00\x00\x00\xff\x2a\x4a\x2d\x2e\xd1\x4d\xcd\x4b\x29\xc8\xcf\xcc\x2b\xd1\x2d\x2d\xca\xb1\x52\xc8\x28\x29\x29\x28\xb6\xd2\xd7\x4f\x2c\xc8\xd4\xcb\xcf\xcd\xcb\x4c\x4d\x49\x4f\xd5\xcb\xcc\x07\xf1\xf5\xcb\x8c\x00\x01\x00\x00\xff\xff\xdf\xc9\x7a\x29\x31\x00\x00\x00")

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

	info := bindataFileInfo{name: "config/prod.yml", size: 49, mode: os.FileMode(420), modTime: time.Unix(1650899779, 0)}
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

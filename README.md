##    

## Login

### Login By Password

```shell
omniedge login -u xxx@xxx.com
```

### Login By Secret-Key

You can generate secret-key on omniedge web.

```shell
omniedge login -s xxxxxx
```

## Join

you can just call `omniedge join`, it will automatically prompt 
the available network for you to choose. And you can 
also add one parameter `-n` to specify the network id manually.

And then, enjoy the omniedge network.

```shell
omniedge join 
// or
omniedge join -n "virtual-network-id" 
```
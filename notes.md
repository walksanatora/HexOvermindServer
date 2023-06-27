# client server sided configs
- global_cordinator 
    - string|false
    - the global cordination server to use, false to only use local storage
- allow_items_global
    - boolean
    - whether items can be pulled or pushed to global server 
    - (requires hexal)
- blacklist_drop_nbt
    - [item id...]
    - list of items that gets their NBT deleted when sent or recieved
- blacklist_import_items
    - [item id...]
    - blacklist of items that cannot be pulled from the global cordinator
- blacklist_import_tags 
    - [item tag...]
    - blacklist of item tags that cannot be pulled from global cordinator
- invert_drop_nbt
    - bool
    - turns drop nbt into a blacklist (will delete nbt on all items *except* these)
- invert_import_items = bool
    - bool
    - turns the itemid blacklist into a whitelist (only allows those items into the server)
- invert_import_tags
    - bool
    - turns the tag blacklist into a whitelist

# overmind server sided configs
time till "death" -> how long untill a iota gets deleted from it's creation time, currently hard coded to 1 hour

TODO: setup filtering for patterns, prepared statments should protect from NBT-based injection
filtered patterns are *only* 256 chars long and *only* consist of `qweasd`
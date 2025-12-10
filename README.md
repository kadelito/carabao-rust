# Carabao

Carabao is a statically-typed scripting language I designed
to make easy things easy.

## Installation

## Example

```.
func split(string str, char delim): string[] {
    new string[] list = []
    new temp = ""
    new float count = 0
    for i in str.indices() {
        new c = str[i]
        if c == delim {
            list.push(temp)
            count = count + 1
            temp = ""
        } else if c == '\0'
            break 1 // break argument is 1 by default
        else
            temp = temp + c
    }
    // Don't forget the last string
    list.push(temp)
    count = count + 1
    print("Split ", str, " into ", count as int, " strings.\n")
    // Note: you can just call println() instead
    return list
}
```

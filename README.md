# The MacHiver

Swiss army knife of some random archiving tools.

## Getting Started

Build the image:

``` bash
docker build -t machiver .
```

## Generate a Data Directory of Photos

Moves photos from an existing source into a new directory, creating a new UUID name, and organized according to the date
the photo was taken.

### Test Run

``` bash
docker run --rm -v $(pwd):/data -it machiver archive-photo -iR
```

### Move Files


``` bash
docker run --rm -v $(pwd):/data -it machiver archive-photo -R -x heic -a output
docker run --rm -v $(pwd):/data -it machiver archive-photo -R -x jpeg -a output
docker run --rm -v $(pwd):/data -it machiver archive-photo -R -x jpg -a output
docker run --rm -v $(pwd):/data -it machiver archive-photo -R -x mov -a output
docker run --rm -v $(pwd):/data -it machiver archive-photo -R -x mp4 -a output
docker run --rm -v $(pwd):/data -it machiver archive-photo -R -x png -a output
```

## Dedup a Bag

### Find Duplicates (in bag only)

``` bash
docker run --rm -v $(pwd):/data -it machiver dedup-bag laptop-bag
```

### Find Duplicates Using a Different Manifest

``` bash
docker run --rm -v $(pwd):/data -it machiver dedup-bag -m manifest-md5.txt laptop-bag
```

### Remove Duplicates

``` bash
docker run --rm -v $(pwd):/data -it machiver dedup-bag -x laptop-bag
```

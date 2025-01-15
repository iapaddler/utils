CWD = $(shell pwd)
OS=linux
CC=gcc
AR=ar
SRC=sensorApi.c common.c bmp3.c bmp388_i2c.c
DEBUG=-g
LIBRSD=librsd.a

librsd: sensorApi.c common.c bmp3.c bmp388_i2c.c
	$(CC) -c $(DEBUG) $(SRC)
	$(AR) rcs $(LIBRSD) *.o

clean:
	rm *.o
	rm *.a

#!/usr/bin/env python3
"""Frame MD5s of an MPEG-1/2 elementary stream decoded by libmpeg2 (the system's
libmpeg2.so.0, through ctypes; test oracle only, never linked into Specular), in the form of
`ffmpeg -f framemd5` (display order, planar 4:2:0 cropped to the coded size).

libavcodec refuses MPEG-1 D-pictures; libmpeg2 decodes them. Usage:
  python3 -I tools/corpus/mpeg1-libmpeg2-oracle.py STREAM > STREAM.framemd5
"""
import ctypes
import hashlib
import sys

lib = ctypes.CDLL("libmpeg2.so.0")


class Fbuf(ctypes.Structure):
    _fields_ = [("buf", ctypes.POINTER(ctypes.c_uint8) * 3), ("id", ctypes.c_void_p)]


class Sequence(ctypes.Structure):
    _fields_ = [("width", ctypes.c_uint), ("height", ctypes.c_uint),
                ("chroma_width", ctypes.c_uint), ("chroma_height", ctypes.c_uint)]


class Info(ctypes.Structure):
    _fields_ = [("sequence", ctypes.POINTER(Sequence)), ("gop", ctypes.c_void_p),
                ("current_picture", ctypes.c_void_p), ("current_picture_2nd", ctypes.c_void_p),
                ("current_fbuf", ctypes.POINTER(Fbuf)), ("display_picture", ctypes.c_void_p),
                ("display_picture_2nd", ctypes.c_void_p), ("display_fbuf", ctypes.POINTER(Fbuf)),
                ("discard_fbuf", ctypes.POINTER(Fbuf)), ("user_data", ctypes.c_void_p),
                ("user_data_len", ctypes.c_uint)]


lib.mpeg2_init.restype = ctypes.c_void_p
lib.mpeg2_info.restype = ctypes.POINTER(Info)
lib.mpeg2_info.argtypes = [ctypes.c_void_p]
lib.mpeg2_buffer.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p]
lib.mpeg2_parse.argtypes = [ctypes.c_void_p]
lib.mpeg2_close.argtypes = [ctypes.c_void_p]
STATE_BUFFER, STATE_SLICE, STATE_END, STATE_INVALID_END = 0, 7, 8, 10

data = open(sys.argv[1], "rb").read()
buf = ctypes.create_string_buffer(data, len(data))
dec = lib.mpeg2_init()
info = lib.mpeg2_info(dec).contents
fed = False
n = 0
while True:
    state = lib.mpeg2_parse(dec)
    if state == STATE_BUFFER:
        if fed:
            break
        start = ctypes.addressof(buf)
        lib.mpeg2_buffer(dec, start, start + len(data))
        fed = True
    elif state in (STATE_SLICE, STATE_END, STATE_INVALID_END) and info.display_fbuf:
        s = info.sequence.contents
        f = info.display_fbuf.contents
        w, h, cw, ch = s.width, s.height, s.chroma_width, s.chroma_height
        out = ctypes.string_at(f.buf[0], w * h) + ctypes.string_at(f.buf[1], cw * ch) \
            + ctypes.string_at(f.buf[2], cw * ch)
        if len(sys.argv) > 2:
            open(sys.argv[2], "ab").write(out)
        print(f"0, {n:10}, {n:10}, {1:8}, {len(out):8}, {hashlib.md5(out).hexdigest()}")
        n += 1
        if state != STATE_SLICE:
            break
lib.mpeg2_close(dec)

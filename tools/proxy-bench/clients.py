import asyncio, struct, sys, time, uuid
N=int(sys.argv[1]); T=float(sys.argv[2])
def vi(n):
    o=b""
    while True:
        b=n&0x7f; n>>=7; o+=bytes([b|(0x80 if n else 0)])
        if not n: return o
def pk(d): return vi(len(d))+d
total=0
async def one(i):
    global total
    r,w=await asyncio.open_connection("127.0.0.1",25590)
    h=vi(0)+vi(777)+vi(9)+b"127.0.0.1"+struct.pack(">H",25590)+vi(2)
    name=f"P{i}".encode()
    w.write(pk(h)+pk(vi(0)+vi(len(name))+name+uuid.uuid4().bytes)); await w.drain()
    end=time.time()+T
    while time.time()<end:
        d=await r.read(262144)
        if not d: break
        total+=len(d)
    w.close()
async def main():
    t0=time.time(); await asyncio.gather(*(one(i) for i in range(N)))
    print(f"{N} connections: {total/(time.time()-t0)/1e6:.0f} MB/s")
asyncio.run(main())

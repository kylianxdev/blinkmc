import asyncio, json, struct
def vi(n):
    o=b""
    while True:
        b=n&0x7f; n>>=7; o+=bytes([b|(0x80 if n else 0)])
        if not n: return o
async def rvi(r):
    n=sh=0
    while True:
        b=(await r.readexactly(1))[0]; n|=(b&0x7f)<<sh; sh+=7
        if not b&0x80: return n
async def packet(r): return await r.readexactly(await rvi(r))
CHUNK=b"\0"*65536
async def handle(r,w):
    try:
        hs=await packet(r); state=hs[-1]
        if state==1:
            await packet(r)
            js=json.dumps({"version":{"name":"Paper 26.3","protocol":777},"players":{"max":100,"online":1},"description":{"text":"bench"}}).encode()
            body=vi(0)+vi(len(js))+js; w.write(vi(len(body))+body); await w.drain()
            p=await packet(r); w.write(vi(len(p))+p); await w.drain()
        else:
            await packet(r)
            while True:
                w.write(CHUNK); await w.drain()
    except Exception: pass
    finally: w.close()
async def main():
    s=await asyncio.start_server(handle,"127.0.0.1",25591); await s.serve_forever()
asyncio.run(main())

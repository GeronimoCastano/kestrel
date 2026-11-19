import queue
import threading

def run(n):
    channel = queue.SimpleQueue()
    result = [0]
    def produce():
        for i in range(n):
            channel.put(i)
        channel.put(None)
    def consume():
        total = 0
        while (x := channel.get()) is not None:
            total += x
        result[0] = total
    threads = [threading.Thread(target=produce), threading.Thread(target=consume)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    return result[0]

print(run(5_000_000))

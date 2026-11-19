import java.util.concurrent.LinkedBlockingQueue;

public class Channels {
    static final Long DONE = Long.MIN_VALUE;
    public static void main(String[] args) throws Exception {
        long n = 5_000_000L;
        LinkedBlockingQueue<Long> queue = new LinkedBlockingQueue<>();
        long[] total = new long[1];
        Thread producer = new Thread(() -> {
            for (long i = 0; i < n; i++) queue.add(i);
            queue.add(DONE);
        });
        Thread consumer = new Thread(() -> {
            long sum = 0;
            try {
                for (Long x = queue.take(); x != DONE; x = queue.take()) sum += x;
            } catch (InterruptedException e) { throw new RuntimeException(e); }
            total[0] = sum;
        });
        producer.start(); consumer.start();
        producer.join(); consumer.join();
        System.out.println(total[0]);
    }
}

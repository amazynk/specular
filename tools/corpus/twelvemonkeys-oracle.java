// Test oracle: decode pictures with Java ImageIO plus the TwelveMonkeys plug-ins (PICT, IFF)
// and write each as `<out>/<name>.rgba`: a "W H\n" text line, then straight RGBA8 rows.
// Prints `<name>\tOK\tW\tH` or `<name>\tERR\t<why>` per file.
//
//   java -Djava.io.tmpdir=<scratch> -cp '<jars>/*' tools/corpus/twelvemonkeys-oracle.java \
//       <out-dir> <file>...
//
// The jars come from tools/corpus/fetch-twelvemonkeys.sh. Nothing here is linked into
// Specular.

import java.awt.image.BufferedImage;
import java.io.File;
import java.io.FileOutputStream;
import java.io.OutputStream;
import javax.imageio.ImageIO;

public class Oracle {
    public static void main(String[] args) throws Exception {
        ImageIO.setUseCache(false);
        ImageIO.scanForPlugins();
        File out = new File(args[0]);
        for (int i = 1; i < args.length; i++) {
            File f = new File(args[i]);
            String name = f.getName();
            try {
                BufferedImage img = ImageIO.read(f);
                if (img == null) {
                    System.out.println(name + "\tERR\tno reader");
                    continue;
                }
                int w = img.getWidth(), h = img.getHeight();
                byte[] px = new byte[w * h * 4];
                int[] row = new int[w];
                for (int y = 0; y < h; y++) {
                    img.getRGB(0, y, w, 1, row, 0, w);
                    for (int x = 0; x < w; x++) {
                        int v = row[x], o = (y * w + x) * 4;
                        px[o] = (byte) (v >> 16);
                        px[o + 1] = (byte) (v >> 8);
                        px[o + 2] = (byte) v;
                        px[o + 3] = (byte) (v >>> 24);
                    }
                }
                try (OutputStream s = new FileOutputStream(new File(out, name + ".rgba"))) {
                    s.write((w + " " + h + "\n").getBytes("US-ASCII"));
                    s.write(px);
                }
                System.out.println(name + "\tOK\t" + w + "\t" + h);
            } catch (Throwable e) {
                System.out.println(name + "\tERR\t" + e);
            }
        }
    }
}

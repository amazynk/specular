// Dumps the first image of an InfraTec .irb in degrees Celsius through the Apache-2.0 `irb`
// reader (de.labathome.irb), as the oracle for specular-fmt-thermal. Run by gen/oracle.py:
//
//     java -cp IRB_CLASSES:gson.jar:. IrbDump FILE.irb OUT.txt
//
// OUT.txt: "width height" on the first line, then every pixel, row-major.

import de.labathome.irb.IrbFile;
import de.labathome.irb.IrbImage;
import java.io.PrintWriter;
import java.util.Locale;

public class IrbDump {
    public static void main(String[] args) throws Exception {
        IrbFile f = IrbFile.fromFile(args[0]);
        IrbImage img = f.images.get(0);
        float[][] c = img.getCelsiusImage();
        try (PrintWriter w = new PrintWriter(args[1], "UTF-8")) {
            w.println(img.width + " " + img.height);
            for (float[] row : c) {
                StringBuilder s = new StringBuilder();
                for (float v : row) {
                    s.append(String.format(Locale.ROOT, "%.6f ", v));
                }
                w.println(s);
            }
        }
    }
}

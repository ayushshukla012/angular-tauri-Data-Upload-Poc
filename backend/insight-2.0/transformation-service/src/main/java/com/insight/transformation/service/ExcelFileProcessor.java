package com.insight.transformation.service;

import org.apache.poi.openxml4j.opc.OPCPackage;
import org.apache.poi.ss.util.CellReference;
import org.apache.poi.ooxml.util.SAXHelper;
import org.apache.poi.xssf.eventusermodel.ReadOnlySharedStringsTable;
import org.apache.poi.xssf.eventusermodel.XSSFReader;
import org.apache.poi.xssf.eventusermodel.XSSFSheetXMLHandler;
import org.apache.poi.xssf.eventusermodel.XSSFSheetXMLHandler.SheetContentsHandler;
import org.apache.poi.xssf.model.StylesTable;
import org.apache.poi.xssf.usermodel.XSSFComment;
import org.springframework.stereotype.Component;
import org.xml.sax.ContentHandler;
import org.xml.sax.InputSource;
import org.xml.sax.XMLReader;

import java.io.IOException;
import java.io.InputStream;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Streams the first sheet of an .xlsx workbook via POI's SAX-based event API instead of
 * loading the workbook into memory (the standard {@code XSSFWorkbook} model would not survive
 * a multi-million-row file).
 */
@Component
public class ExcelFileProcessor implements FileProcessorStrategy {

    @Override
    public String fileType() {
        return "EXCEL";
    }

    @Override
    public void process(InputStream inputStream, FileRowHandler rowHandler) throws IOException {
        try (OPCPackage pkg = OPCPackage.open(inputStream)) {
            XSSFReader reader = new XSSFReader(pkg);
            ReadOnlySharedStringsTable sharedStrings = new ReadOnlySharedStringsTable(pkg);
            StylesTable styles = reader.getStylesTable();

            XMLReader xmlReader = SAXHelper.newXMLReader();
            ContentHandler contentHandler = new XSSFSheetXMLHandler(
                    styles, sharedStrings, new RowCollector(rowHandler), false);
            xmlReader.setContentHandler(contentHandler);

            Iterator<InputStream> sheets = reader.getSheetsData();
            if (sheets.hasNext()) {
                try (InputStream sheetStream = sheets.next()) {
                    xmlReader.parse(new InputSource(sheetStream));
                }
            }
        } catch (Exception e) {
            throw new IOException("Failed to parse Excel file", e);
        }
    }

    private static final class RowCollector implements SheetContentsHandler {

        private final FileRowHandler rowHandler;
        private final List<String> headers = new ArrayList<>();
        private Map<String, String> currentRow;
        private long rowNumber = 0;
        private boolean firstRow = true;

        private RowCollector(FileRowHandler rowHandler) {
            this.rowHandler = rowHandler;
        }

        @Override
        public void startRow(int rowNum) {
            currentRow = new LinkedHashMap<>();
        }

        @Override
        public void endRow(int rowNum) {
            if (firstRow) {
                firstRow = false;
                return;
            }
            rowNumber++;
            rowHandler.onRow(rowNumber, currentRow);
        }

        @Override
        public void cell(String cellReference, String formattedValue, XSSFComment comment) {
            int columnIndex = new CellReference(cellReference).getCol();
            if (firstRow) {
                while (headers.size() <= columnIndex) {
                    headers.add("");
                }
                headers.set(columnIndex, formattedValue);
            } else {
                String key = columnIndex < headers.size() ? headers.get(columnIndex) : "column" + columnIndex;
                currentRow.put(key, formattedValue);
            }
        }
    }
}

import { Reports } from "ui";
const queryString = require('query-string');

export function get_reports() {
  const reports_arg = queryString.parse(location.search).reports;
  if (reports_arg) {
    return Reports.from_str(reports_arg);
  } else {
    return Reports.new();
  }
}
import { Combobox, InputBase, InputBaseProps, PolymorphicComponentProps, useCombobox } from '@mantine/core';
import { useRef, useState } from 'react';

export function SelectCreatable({ defaultValue, choices, onSubmit, inputBaseProps = {} }: { defaultValue?: string, choices: string[], onSubmit: (value: string) => void, inputBaseProps?: PolymorphicComponentProps<'input', InputBaseProps> }) {
  const combobox = useCombobox({
    onDropdownClose: () => combobox.resetSelectedOption(),
  });

  const [data, setData] = useState(choices);
  const [value, setValue] = useState<string | null>(defaultValue || null);
  const [search, setSearch] = useState(defaultValue || '');

  const exactOptionMatch = data.some((item) => item === search);
  const filteredOptions = exactOptionMatch
    ? data
    : data.filter((item) => item.toLowerCase().includes(search.toLowerCase().trim()));

  const options = filteredOptions.map((item) => (
    <Combobox.Option value={item} key={item}>
      {item}
    </Combobox.Option>
  ));

  const inputRef = useRef<HTMLInputElement | null>(null);

  return (
    <Combobox
      store={combobox}
      withinPortal={false}
      onOptionSubmit={val => {
        if (inputRef.current?.reportValidity()) {
          if (val === '$create') {
            setData(current => [...current, search]);
            setValue(search);
            onSubmit(search);
          } else {
            setValue(val);
            setSearch(val);
            onSubmit(val);
          }
        }
        combobox.closeDropdown();
      }}
    >
      <Combobox.Target>
        <InputBase
          ref={inputRef}
          rightSection={<Combobox.Chevron />}
          value={search}
          onChange={(event) => {
            combobox.openDropdown();
            combobox.updateSelectedOptionIndex();
            setSearch(event.currentTarget.value);
          }}
          onClick={() => combobox.openDropdown()}
          onFocus={() => combobox.openDropdown()}
          onBlur={() => {
            combobox.closeDropdown();
            setSearch(value || '');
          }}
          placeholder='Search value'
          rightSectionPointerEvents='none'
          {...inputBaseProps}
        />
      </Combobox.Target>

      <Combobox.Dropdown>
        <Combobox.Options>
          {options}
          {!exactOptionMatch && search.trim().length > 0 && (
            <Combobox.Option value="$create">+ {search}</Combobox.Option>
          )}
        </Combobox.Options>
      </Combobox.Dropdown>
    </Combobox>
  );
}
